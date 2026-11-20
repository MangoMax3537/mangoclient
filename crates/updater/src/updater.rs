use crate::error::{Result, UpdateError};
use crate::installer::Installer;
use crate::manifest::{UpdateManifest, UrlPolicy};
use crate::platform::Platform;
use crate::state::{DisabledReason, UpdateState, UpdateStatus};
use crate::verify::{verify_update_or_delete, UPDATE_PUBLIC_KEY_B64};
use mango_net::{download_file, get_bytes, DownloadOptions, Expected, FetchOptions};
use semver::Version;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Players leave a launcher open for days, so re-check while it runs.
pub const RECHECK_INTERVAL: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Debug, Clone)]
pub struct UpdaterConfig {
    pub manifest_url: String,
    pub current_version: Version,
    /// Where verified updates are kept until installed.
    pub download_dir: PathBuf,
    pub public_key_b64: String,
    pub platform: Platform,
    pub policy: UrlPolicy,
    pub fetch: FetchOptions,
    /// Unpackaged dev run: updates are disabled.
    pub dev: bool,
    /// Offer prerelease versions (default off: stable never moves to a prerelease).
    pub allow_prerelease: bool,
}

impl UpdaterConfig {
    pub fn new(
        manifest_url: impl Into<String>,
        current_version: Version,
        download_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            manifest_url: manifest_url.into(),
            current_version,
            download_dir: download_dir.into(),
            public_key_b64: UPDATE_PUBLIC_KEY_B64.into(),
            platform: Platform::detect(),
            policy: UrlPolicy::default(),
            fetch: FetchOptions::default(),
            dev: false,
            allow_prerelease: false,
        }
    }
}

struct Inner {
    cfg: UpdaterConfig,
    installer: Arc<dyn Installer>,
    status: watch::Sender<UpdateStatus>,
    relaunch: watch::Sender<bool>,
    applying: AtomicBool,
}

/// Signed-update state machine. Cheap to clone.
#[derive(Clone)]
pub struct Updater {
    inner: Arc<Inner>,
}

impl Updater {
    pub fn new(cfg: UpdaterConfig, installer: Arc<dyn Installer>) -> Self {
        let initial = UpdateStatus {
            state: UpdateState::Idle,
            current: cfg.current_version.to_string(),
            applying: false,
        };
        Self {
            inner: Arc::new(Inner {
                cfg,
                installer,
                status: watch::channel(initial).0,
                relaunch: watch::channel(false).0,
                applying: AtomicBool::new(false),
            }),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        self.inner.status.borrow().clone()
    }

    /// Subscribe to state changes (for the `update:state` event).
    pub fn subscribe(&self) -> watch::Receiver<UpdateStatus> {
        self.inner.status.subscribe()
    }

    /// Becomes `true` when a launch check (`apply = true`) has a verified
    /// update staged and the shell should relaunch (the legacy app waited
    /// ~800ms first so the UI could paint).
    pub fn subscribe_relaunch(&self) -> watch::Receiver<bool> {
        self.inner.relaunch.subscribe()
    }

    fn set(&self, state: UpdateState) {
        let applying = self.inner.applying.load(Ordering::SeqCst);
        let current = self.inner.cfg.current_version.to_string();
        self.inner.status.send_replace(UpdateStatus {
            state,
            current,
            applying,
        });
    }

    fn disabled_reason(&self) -> Option<DisabledReason> {
        let c = &self.inner.cfg;
        if c.dev {
            Some(DisabledReason::Dev)
        } else if !c.platform.supported() {
            Some(DisabledReason::Unsupported)
        } else {
            None
        }
    }

    /// Atomically move to Checking unless a check/download/staged update exists.
    fn try_begin(&self) -> bool {
        let applying = self.inner.applying.load(Ordering::SeqCst);
        let current = self.inner.cfg.current_version.to_string();
        self.inner.status.send_if_modified(|s| {
            if matches!(
                s.state,
                UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Ready { .. }
            ) {
                return false;
            }
            *s = UpdateStatus {
                state: UpdateState::Checking,
                current,
                applying,
            };
            true
        })
    }

    /// Check for an update. `apply` is for the check at launch: once the
    /// update is verified and staged, the relaunch signal fires. Periodic
    /// checks pass `false` and only offer.
    pub async fn check(&self, apply: bool) -> UpdateStatus {
        if let Some(reason) = self.disabled_reason() {
            self.inner.applying.store(false, Ordering::SeqCst);
            self.set(UpdateState::Disabled { reason });
            return self.status();
        }
        if !self.try_begin() {
            return self.status();
        }
        self.inner.applying.store(apply, Ordering::SeqCst);
        self.set(UpdateState::Checking);
        match self.run_check().await {
            Ok(state) => {
                let ready = matches!(state, UpdateState::Ready { .. });
                self.set(state);
                if ready && apply {
                    self.inner.relaunch.send_replace(true);
                }
            }
            Err(err) => {
                // Never strand the player on the update screen.
                self.inner.applying.store(false, Ordering::SeqCst);
                let message: String = err.to_string().chars().take(200).collect();
                self.set(UpdateState::Error {
                    code: err.kind(),
                    message,
                });
            }
        }
        self.status()
    }

    /// Hand a ready update to the installer (the "Restart now" button).
    pub fn install(&self) -> Result<bool> {
        if !matches!(self.status().state, UpdateState::Ready { .. }) {
            return Ok(false);
        }
        self.inner.installer.apply_on_quit()?;
        Ok(true)
    }

    /// Re-check every [`RECHECK_INTERVAL`] (offer only). Abort the handle to stop.
    pub fn spawn_periodic(&self, every: Duration) -> tokio::task::JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(tick_loop(every, move || {
            let this = this.clone();
            async move {
                this.check(false).await;
            }
        }))
    }

    async fn run_check(&self) -> Result<UpdateState> {
        let cfg = &self.inner.cfg;
        cfg.policy.check(&cfg.manifest_url)?;
        let bytes = get_bytes(&cfg.manifest_url, &cfg.fetch).await?;
        let manifest = UpdateManifest::parse(&bytes, &cfg.policy)?;
        if !is_newer(
            &manifest.version,
            &cfg.current_version,
            cfg.allow_prerelease,
        ) {
            return Ok(UpdateState::Current);
        }
        let keys = cfg.platform.manifest_keys();
        let (platform_key, entry) = keys
            .iter()
            .find_map(|k| manifest.platforms.get(k).map(|e| (k.as_str(), e)))
            .ok_or_else(|| UpdateError::NoPlatformBuild(keys.join("|")))?;

        let version = manifest.version.to_string();
        self.set(UpdateState::Downloading {
            version: version.clone(),
            percent: 0,
        });

        let dest = cfg.download_dir.join(file_name(&version, &entry.url));
        let progress = {
            let this = self.clone();
            let version = version.clone();
            let last = Arc::new(std::sync::atomic::AtomicU8::new(0));
            Arc::new(move |done: u64, total: u64| {
                if total == 0 {
                    return;
                }
                let pct = (done.saturating_mul(100) / total).min(100) as u8;
                if last.swap(pct, Ordering::Relaxed) != pct {
                    this.set(UpdateState::Downloading {
                        version: version.clone(),
                        percent: pct,
                    });
                }
            }) as mango_net::ProgressFn
        };
        let opts = DownloadOptions {
            expected: Expected {
                sha256: Some(entry.sha256.to_lowercase()),
                size: Some(entry.size),
                ..Expected::default()
            },
            progress: Some(progress),
            cache: None,
            fetch: cfg.fetch.clone(),
        };
        download_file(&entry.url, &dest, &opts).await?;
        // Hash and size match; now require the publisher's signature over
        // version + platform + hash (not just the file bytes).
        verify_update_or_delete(
            &cfg.public_key_b64,
            &entry.signature,
            &version,
            platform_key,
            &dest,
        )
        .await?;
        if let Err(e) = self.inner.installer.stage(&dest) {
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(e);
        }
        Ok(UpdateState::Ready { version })
    }
}

/// Never downgrade; stable builds ignore prereleases unless allowed.
pub fn is_newer(candidate: &Version, current: &Version, allow_prerelease: bool) -> bool {
    if !allow_prerelease && !candidate.pre.is_empty() {
        return false;
    }
    candidate.cmp_precedence(current).is_gt()
}

fn file_name(version: &str, url: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or("");
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    let name = clean(last);
    let name = if name.trim_matches('.').is_empty() {
        "update.bin".to_string()
    } else {
        name
    };
    format!("mangoclient-{}-{name}", clean(version))
}

/// Interval driver, separate from the network so it is testable with a paused clock.
pub(crate) async fn tick_loop<F, Fut>(every: Duration, mut tick: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    let mut iv = tokio::time::interval(every);
    iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    iv.tick().await; // the first tick is immediate; the launch check covers it
    loop {
        iv.tick().await;
        tick().await;
    }
}

//! Startup work (`AppService::start_background`), telemetry, the updater and
//! the `mods/` watcher.

use crate::error::Res;
use crate::events::{ContentChangedEvent, Event};
use crate::models::UpdateStatus;
use crate::service::{lock, AppService, Inner};
use mango_mango::telemetry;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Weak};

impl AppService {
    /// Telemetry, update checks and the `mods/` watcher. Needs a tokio
    /// runtime; call it once after the service is managed. Every task holds
    /// only a weak reference, so they end with the service.
    pub fn start_background(&self) {
        self.sync_telemetry();
        self.sync_desktop_integration();
        self.spawn_update_events();
        if self.inner.opts.auto_update_check {
            let updater = self.inner.updater.clone();
            tokio::spawn({
                let updater = updater.clone();
                // Download and stage only: the banner offers "Restart now", and a normal quit installs the staged update
                // (`update_install_on_exit`), so the new version starts with the next launch. Never restarts by itself.
                async move {
                    updater.check(false).await;
                }
            });
            updater.spawn_periodic(mango_updater::RECHECK_INTERVAL);
        }
        self.spawn_mods_watcher();
        self.spawn_skin_prefetch();
    }

    /// Linux AppImage: add, refresh (the AppImage path changes with updates) or remove the menu entry to match
    /// `config.desktopIntegration`. Before the first-run setup is done nothing is added (the setup asks). Never fails
    /// the caller.
    pub(crate) fn sync_desktop_integration(&self) {
        let Some(d) = crate::desktop_entry::DesktopIntegration::detect() else {
            return;
        };
        let (on, setup_done) = self
            .inner
            .with_store(|s| (s.config.desktop_integration, s.config.first_run_done));
        let result = if !on {
            d.remove().map(|_| ())
        } else if setup_done {
            d.install().map(|_| ())
        } else {
            Ok(())
        };
        if let Err(e) = result {
            tracing::warn!("desktop integration: {e}");
        }
    }

    /// Start or stop telemetry to match `config.telemetry`. A no-op without a
    /// runtime (the next call with one catches up).
    pub(crate) fn sync_telemetry(&self) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let want = self.inner.with_store(|s| s.config.telemetry);
        let mut slot = lock(&self.inner.telemetry);
        match (want, slot.is_some()) {
            (true, false) => {
                let base = self
                    .inner
                    .opts
                    .api_base
                    .clone()
                    .unwrap_or_else(telemetry::base_from_env);
                let weak: Weak<Inner> = Arc::downgrade(&self.inner);
                *slot = Some(telemetry::start(
                    &self.inner.paths,
                    base,
                    self.inner.opts.app_version.clone(),
                    // Asked before every beat: the opt-out needs no restart.
                    move || {
                        weak.upgrade()
                            .is_some_and(|i| i.with_store(|s| s.config.telemetry))
                    },
                ));
            }
            (false, true) => {
                if let Some(t) = slot.take() {
                    t.stop();
                }
            }
            _ => {}
        }
    }

    /// Is the telemetry task running?
    pub fn telemetry_running(&self) -> bool {
        lock(&self.inner.telemetry).is_some()
    }

    fn spawn_update_events(&self) {
        let mut rx = self.inner.updater.subscribe();
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let status = UpdateStatus::from(&*rx.borrow_and_update());
                let Some(inner) = weak.upgrade() else { return };
                inner.emit(Event::UpdateStatus(status));
            }
        });
    }

    pub fn update_status(&self) -> UpdateStatus {
        UpdateStatus::from(&self.inner.updater.status())
    }

    pub async fn update_check(&self) -> Res<UpdateStatus> {
        Ok(UpdateStatus::from(&self.inner.updater.check(false).await))
    }

    /// `true` when the staged update was handed to the installer.
    pub fn update_install(&self) -> Res<bool> {
        Ok(self.inner.updater.install()?)
    }

    /// A normal quit with a staged update: install it without starting the
    /// new version. `true` when something was installed.
    pub fn update_install_on_exit(&self) -> Res<bool> {
        Ok(self.inner.updater.install_on_exit()?)
    }

    /// Poll the `mods/` folder of the selected and the running profiles and
    /// emit `content:changed` when it differs from the last look.
    fn spawn_mods_watcher(&self) {
        let weak = Arc::downgrade(&self.inner);
        let every = self.inner.opts.watch_interval;
        tokio::spawn(async move {
            let mut seen: HashMap<String, u64> = HashMap::new();
            loop {
                tokio::time::sleep(every).await;
                let Some(inner) = weak.upgrade() else { return };
                let mut ids: Vec<String> = lock(&inner.games).keys().cloned().collect();
                if let Some(id) = inner.with_store(|s| s.selected_profile().map(|p| p.id.clone())) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                let paths = inner.paths.clone();
                let probe = ids.clone();
                let sigs = tokio::task::spawn_blocking(move || {
                    probe
                        .into_iter()
                        .filter_map(|id| {
                            let dir = paths.instance_dir(&id).ok()?.join("mods");
                            Some((id, folder_signature(&dir)))
                        })
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap_or_default();
                seen.retain(|id, _| ids.contains(id));
                for (id, sig) in sigs {
                    // The first look is the baseline, not a change.
                    if seen.insert(id.clone(), sig).is_some_and(|old| old != sig) {
                        inner.emit(Event::ContentChanged(ContentChangedEvent {
                            profile_id: id,
                        }));
                    }
                }
            }
        });
    }
}

/// Hash of the (name, size, mtime) of every entry in `dir`.
pub(crate) fn folder_signature(dir: &Path) -> u64 {
    let mut entries: Vec<(String, u64, u128)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let meta = std::fs::symlink_metadata(e.path()).ok()?;
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_nanos());
                    Some((
                        e.file_name().to_string_lossy().into_owned(),
                        meta.len(),
                        mtime,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    let mut h = DefaultHasher::new();
    entries.hash(&mut h);
    h.finish()
}

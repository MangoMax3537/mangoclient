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
        self.spawn_update_events();
        if self.inner.opts.auto_update_check {
            let updater = self.inner.updater.clone();
            self.spawn_update_relaunch();
            tokio::spawn({
                let updater = updater.clone();
                // The launch check applies: a verified update is handed to
                // the installer, which restarts into the new version.
                async move {
                    updater.check(true).await;
                }
            });
            updater.spawn_periodic(mango_updater::RECHECK_INTERVAL);
        }
        self.spawn_mods_watcher();
        self.spawn_skin_prefetch();
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

    /// Launch-time update: once the check has a verified, staged update,
    /// wait a moment (so the UI paints) and hand it to the installer.
    fn spawn_update_relaunch(&self) {
        let mut rx = self.inner.updater.subscribe_relaunch();
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if !*rx.borrow_and_update() {
                    continue;
                }
                let Some(inner) = weak.upgrade() else { return };
                let delay = inner.opts.update_relaunch_delay;
                let updater = inner.updater.clone();
                drop(inner);
                tokio::time::sleep(delay).await;
                // Installers touch the file system: keep it off the reactor.
                let result = tokio::task::spawn_blocking(move || updater.install()).await;
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => tracing::warn!("launch-time update not installed: {e}"),
                    Err(e) => tracing::warn!("launch-time update install panicked: {e}"),
                }
                return;
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

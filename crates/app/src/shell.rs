//! Support for the desktop shell: files dropped on the window and the
//! activity snapshot the window policy needs.

use crate::events::{Event, FilesDroppedEvent};
use crate::path_grants::GrantKind;
use crate::service::{lock, AppService};
use mango_updater::UpdateState;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

/// At most this many dropped files are looked at (the grant list is small).
const MAX_DROPPED: usize = 32;

/// Image extensions of `pick_image_file`.
const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
/// Import extensions of `pick_import_file`.
const IMPORT_EXTS: [&str; 2] = ["mrpack", "zip"];

/// What the window policy needs to know about the service right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Activity {
    pub running_games: usize,
    /// A launch pipeline, install, import, restore or update download is in
    /// flight (the process must not exit under it).
    pub busy: bool,
    pub update_ready: bool,
    pub hide_on_launch: bool,
    pub keep_launcher_open: bool,
}

/// Counts an install-like operation for as long as it lives.
pub(crate) struct BusyGuard<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn grant_kind(path: &Path) -> Option<GrantKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMPORT_EXTS.contains(&ext.as_str()) {
        Some(GrantKind::File)
    } else if IMAGE_EXTS.contains(&ext.as_str()) {
        Some(GrantKind::Image)
    } else {
        None
    }
}

impl AppService {
    pub(crate) fn busy_guard(&self) -> BusyGuard<'_> {
        self.inner.busy_ops.fetch_add(1, Ordering::SeqCst);
        BusyGuard(&self.inner.busy_ops)
    }

    /// Snapshot for the window policy.
    pub fn activity(&self) -> Activity {
        let inner = &self.inner;
        let running_games = lock(&inner.games).len();
        let launching = !lock(&inner.launching).is_empty();
        let state = inner.updater.status().state;
        let downloading = matches!(
            state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        );
        let (hide_on_launch, keep_launcher_open) =
            inner.with_store(|s| (s.config.hide_on_launch, s.config.keep_launcher_open));
        Activity {
            running_games,
            busy: launching || downloading || inner.busy_ops.load(Ordering::SeqCst) > 0,
            update_ready: matches!(state, UpdateState::Ready { .. }),
            hide_on_launch,
            keep_launcher_open,
        }
    }

    /// Files dropped on the window: every regular file with an import (.mrpack,
    /// .zip) or image extension gets the same one-shot grant the pickers
    /// give, and only after that `files:dropped` goes out, so the UI can use
    /// the paths at once. Directories, other file types and UNC/device paths
    /// are ignored. Returns the paths that were announced.
    pub fn files_dropped(&self, dropped: &[PathBuf]) -> Vec<String> {
        let mut paths = Vec::new();
        for p in dropped.iter().take(MAX_DROPPED) {
            let Some(raw) = p.to_str() else { continue };
            if crate::path_grants::reject_special_path(raw).is_err() {
                continue;
            }
            // `metadata` follows links: a link to a regular file is fine, a
            // link to a directory is not.
            if !std::fs::metadata(p).is_ok_and(|m| m.is_file()) {
                continue;
            }
            let Some(kind) = grant_kind(p) else { continue };
            self.inner.grants.grant(kind, p);
            paths.push(raw.to_string());
        }
        if !paths.is_empty() {
            self.inner.emit(Event::FilesDropped(FilesDroppedEvent {
                paths: paths.clone(),
            }));
        }
        paths
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::events::EventSink;
    use crate::host::Host;
    use crate::service::ServiceOptions;
    use mango_core::Paths;
    use std::sync::{Arc, Mutex, OnceLock};

    /// On `files:dropped` it immediately tries to use every announced path
    /// the way the UI would; records what worked.
    #[derive(Default)]
    struct Probe {
        svc: OnceLock<AppService>,
        seen: Mutex<Vec<(String, bool, bool)>>,
    }

    impl EventSink for Probe {
        fn emit(&self, event: Event) {
            let Event::FilesDropped(e) = event else {
                return;
            };
            let Some(svc) = self.svc.get() else { return };
            let mut seen = self.seen.lock().unwrap();
            for p in e.paths {
                let file = svc.checked_in_path(&p, GrantKind::File).is_ok();
                let image = svc.checked_in_path(&p, GrantKind::Image).is_ok();
                seen.push((p, file, image));
            }
        }
    }

    fn strict() -> (tempfile::TempDir, AppService, Arc<Probe>) {
        let tmp = tempfile::tempdir().unwrap();
        let probe = Arc::new(Probe::default());
        let opts = ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            strict_paths: true,
            ..ServiceOptions::default()
        };
        let svc = AppService::with_host(
            Paths::new(tmp.path().join("data")),
            probe.clone(),
            opts,
            Host::system(false),
        )
        .unwrap();
        probe.svc.set(svc.clone()).ok();
        (tmp, svc, probe)
    }

    #[test]
    fn paths_are_granted_before_the_event_goes_out() {
        let (tmp, svc, probe) = strict();
        let pack = tmp.path().join("pack.MRPACK");
        let pic = tmp.path().join("pic.png");
        std::fs::write(&pack, b"x").unwrap();
        std::fs::write(&pic, b"x").unwrap();
        // Not announced yet: nothing is granted, so strict mode refuses.
        assert!(svc
            .checked_in_path(pack.to_str().unwrap(), GrantKind::File)
            .is_err());
        let out = svc.files_dropped(&[pack.clone(), pic.clone()]);
        assert_eq!(out.len(), 2);
        let seen = probe.seen.lock().unwrap().clone();
        // The sink could use each path inside `emit`: the grant came first,
        // with the kind that matches the extension.
        assert_eq!(seen[0], (pack.display().to_string(), true, false));
        assert_eq!(seen[1], (pic.display().to_string(), false, true));
    }

    #[test]
    fn directories_other_types_and_missing_files_are_ignored() {
        let (tmp, svc, probe) = strict();
        let dir = tmp.path().join("folder.zip");
        std::fs::create_dir(&dir).unwrap();
        let txt = tmp.path().join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        let none = tmp.path().join("gone.png");
        let noext = tmp.path().join("README");
        std::fs::write(&noext, b"x").unwrap();
        let out = svc.files_dropped(&[dir, txt.clone(), none, noext]);
        assert!(out.is_empty());
        // No event at all, and nothing was granted.
        assert!(probe.seen.lock().unwrap().is_empty());
        assert!(svc
            .checked_in_path(txt.to_str().unwrap(), GrantKind::File)
            .is_err());
    }

    #[test]
    fn a_grant_is_not_enough_for_the_other_kind_and_expires_after_use() {
        let (tmp, svc, _probe) = strict();
        let pic = tmp.path().join("a.jpeg");
        std::fs::write(&pic, b"x").unwrap();
        svc.files_dropped(std::slice::from_ref(&pic));
        let raw = pic.to_str().unwrap();
        // The probe used up the image grant (2 uses) once; the rest is limited.
        assert!(svc.checked_in_path(raw, GrantKind::File).is_err());
        assert!(svc.checked_in_path(raw, GrantKind::Image).is_ok());
        assert!(svc.checked_in_path(raw, GrantKind::Image).is_err());
    }

    #[test]
    fn busy_guard_counts_and_releases() {
        let (_tmp, svc, _p) = strict();
        assert!(!svc.activity().busy);
        {
            let _g = svc.busy_guard();
            assert!(svc.activity().busy);
        }
        assert!(!svc.activity().busy);
    }
}

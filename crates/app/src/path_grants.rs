//! One-shot allow-list of paths the user chose in a native dialog, and the
//! hardening every user-chosen path goes through.
//!
//! A compromised UI could otherwise hand any path string to commands that
//! write (`instance_export`) or read (`profile_icon_set_image`) files. With
//! `strict_paths` on (the Tauri shell), those commands only accept a path
//! that a `pick_*` command returned to the UI a moment ago. Grants expire
//! and have a small use count.

use crate::error::{AppError, Res};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a picked path stays usable.
pub const DEFAULT_GRANT_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_GRANTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantKind {
    /// A picture for an icon or background.
    Image,
    /// A folder (watched download folders).
    Folder,
    /// A file to write.
    Save,
    /// A file to import (plan, then import).
    File,
}

impl GrantKind {
    fn uses(self) -> u32 {
        match self {
            GrantKind::Image | GrantKind::File => 2,
            GrantKind::Folder | GrantKind::Save => 1,
        }
    }
}

#[derive(Debug)]
struct Grant {
    kind: GrantKind,
    path: PathBuf,
    expires: Instant,
    uses_left: u32,
}

#[derive(Debug)]
pub(crate) struct PathGrants {
    strict: bool,
    ttl: Duration,
    entries: Mutex<Vec<Grant>>,
}

/// Canonical form when the path exists; otherwise canonical parent + name
/// (a file about to be created); otherwise as given.
fn normalize(p: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(p) {
        return c;
    }
    match (
        p.parent().and_then(|d| std::fs::canonicalize(d).ok()),
        p.file_name(),
    ) {
        (Some(dir), Some(name)) => dir.join(name),
        _ => p.to_path_buf(),
    }
}

fn not_granted() -> AppError {
    AppError::new(
        "path.not_granted",
        "Choose the file or folder with the picker first",
    )
}

impl PathGrants {
    pub fn new(strict: bool, ttl: Duration) -> Self {
        Self {
            strict,
            ttl,
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn grant(&self, kind: GrantKind, path: &Path) {
        self.grant_at(kind, path, Instant::now());
    }

    pub fn grant_at(&self, kind: GrantKind, path: &Path, now: Instant) {
        let path = normalize(path);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|g| g.expires > now);
        entries.retain(|g| !(g.kind == kind && g.path == path));
        if entries.len() >= MAX_GRANTS {
            entries.remove(0);
        }
        entries.push(Grant {
            kind,
            path,
            expires: now + self.ttl,
            uses_left: kind.uses(),
        });
    }

    /// Use one grant of `kind` for `raw`. Always `Ok` when not strict.
    pub fn consume(&self, kind: GrantKind, raw: &str) -> Res<()> {
        self.consume_at(kind, raw, Instant::now())
    }

    pub fn consume_at(&self, kind: GrantKind, raw: &str, now: Instant) -> Res<()> {
        if !self.strict {
            return Ok(());
        }
        let path = normalize(Path::new(raw));
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|g| g.expires > now && g.uses_left > 0);
        let Some(g) = entries
            .iter_mut()
            .find(|g| g.kind == kind && g.path == path)
        else {
            return Err(not_granted());
        };
        g.uses_left -= 1;
        entries.retain(|g| g.uses_left > 0);
        Ok(())
    }
}

/// Reject UNC and Windows device paths (`\\server\share`, `\\?\C:\`,
/// `\\.\pipe\x`, `//server/share`), which can reach network shares or
/// devices. Checked on every platform; on Windows the prefix components are
/// inspected as well.
pub(crate) fn reject_special_path(raw: &str) -> Res<()> {
    let bad = raw.starts_with("\\\\") || (cfg!(windows) && raw.starts_with("//"));
    #[cfg(windows)]
    let bad = bad || {
        use std::path::{Component, Prefix};
        matches!(
            Path::new(raw).components().next(),
            Some(Component::Prefix(p)) if !matches!(p.kind(), Prefix::Disk(_))
        )
    };
    if bad {
        return Err(AppError::new(
            "path.unsupported",
            "Network and device paths are not supported",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn grants(strict: bool) -> PathGrants {
        PathGrants::new(strict, Duration::from_secs(600))
    }

    #[test]
    fn granted_path_is_accepted_then_used_up() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("out.zip");
        let g = grants(true);
        g.grant(GrantKind::Save, &f);
        let raw = f.to_str().unwrap();
        assert!(g.consume(GrantKind::Save, raw).is_ok());
        // Save grants are single use.
        assert_eq!(
            g.consume(GrantKind::Save, raw).unwrap_err().code,
            "path.not_granted"
        );
    }

    #[test]
    fn import_grant_allows_plan_and_import_but_no_third_use() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("pack.mrpack");
        std::fs::write(&f, b"x").unwrap();
        let g = grants(true);
        g.grant(GrantKind::File, &f);
        let raw = f.to_str().unwrap();
        assert!(g.consume(GrantKind::File, raw).is_ok());
        assert!(g.consume(GrantKind::File, raw).is_ok());
        assert!(g.consume(GrantKind::File, raw).is_err());
    }

    #[test]
    fn expired_grant_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("pic.png");
        std::fs::write(&f, b"x").unwrap();
        let g = grants(true);
        let t0 = Instant::now();
        g.grant_at(GrantKind::Image, &f, t0);
        let late = t0 + Duration::from_secs(601);
        assert!(g
            .consume_at(GrantKind::Image, f.to_str().unwrap(), late)
            .is_err());
        // Just inside the window it still works.
        g.grant_at(GrantKind::Image, &f, t0);
        assert!(g
            .consume_at(
                GrantKind::Image,
                f.to_str().unwrap(),
                t0 + Duration::from_secs(599)
            )
            .is_ok());
    }

    #[test]
    fn other_paths_and_other_kinds_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let g = grants(true);
        g.grant(GrantKind::Image, &a);
        assert!(g.consume(GrantKind::Image, b.to_str().unwrap()).is_err());
        assert!(g.consume(GrantKind::Save, a.to_str().unwrap()).is_err());
        assert!(g.consume(GrantKind::Image, a.to_str().unwrap()).is_ok());
    }

    #[test]
    fn dotdot_spelling_of_a_granted_path_matches_and_others_do_not() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let a = dir.path().join("a.png");
        std::fs::write(&a, b"x").unwrap();
        let g = grants(true);
        g.grant(GrantKind::Image, &a);
        let spelled = dir.path().join("sub").join("..").join("a.png");
        assert!(g
            .consume(GrantKind::Image, spelled.to_str().unwrap())
            .is_ok());
    }

    #[test]
    fn non_strict_accepts_anything() {
        let g = grants(false);
        assert!(g.consume(GrantKind::Image, "/anything/at/all.png").is_ok());
    }

    #[test]
    fn unc_and_device_paths_are_rejected() {
        for p in [
            "\\\\server\\share\\a.zip",
            "\\\\?\\C:\\a.zip",
            "\\\\.\\pipe\\x",
        ] {
            assert_eq!(
                reject_special_path(p).unwrap_err().code,
                "path.unsupported",
                "{p}"
            );
        }
        assert!(reject_special_path("/home/user/a.zip").is_ok());
        assert!(reject_special_path("C:\\Users\\a.zip").is_ok());
    }
}

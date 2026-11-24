//! Atomic JSON persistence with a last-known-good `.bak` generation.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// `rename`, retried a few times on Windows: an antivirus or the search indexer briefly holding the target makes the
/// replace fail with "access denied" for a moment, which must not lose a save.
fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match fs::rename(from, to) {
            Err(e)
                if cfg!(windows)
                    && e.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt < 5 =>
            {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(30 * attempt));
            }
            r => return r,
        }
    }
}

fn valid_json(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw).is_ok()
}

fn sidecar(file: &Path, suffix: &str) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub fn backup_path(file: &Path) -> PathBuf {
    sidecar(file, ".bak")
}

/// Write a complete file, flush it, then atomically put it in place.
/// `mode` is applied on unix only.
pub fn write_file_atomic(file: &Path, data: &[u8], mode: u32) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = sidecar(file, &format!(".tmp-{}-{nanos}-{n}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, mode);
        #[cfg(not(unix))]
        let _ = mode;
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        rename_retrying(&tmp, file)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(file, fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io(file, e));
    }
    Ok(())
}

/// Read JSON, recovering from `<file>.bak` when the main file is missing or
/// corrupt. Recovery restores only the main file: rotating would overwrite the
/// good backup with the corrupt file. `None` when neither copy is valid.
pub fn read_json_value(file: &Path) -> Option<Value> {
    if let Ok(raw) = fs::read_to_string(file) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            return Some(v);
        }
    }
    let raw = fs::read_to_string(backup_path(file)).ok()?;
    let value = serde_json::from_str::<Value>(&raw).ok()?;
    // Best effort; recovery still works in memory.
    let _ = write_file_atomic(file, raw.as_bytes(), 0o600);
    Some(value)
}

/// Typed read with a fallback for missing/unrecoverable/mismatched data.
pub fn read_json_or<T: serde::de::DeserializeOwned>(file: &Path, fallback: T) -> T {
    read_json_value(file)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or(fallback)
}

/// Pretty JSON (2 spaces) with a trailing newline. The previous valid
/// generation becomes `<file>.bak`; on the first save both copies are written
/// (backup first), so either one is sufficient after a crash.
pub fn write_json_atomic<T: Serialize>(file: &Path, data: &T) -> Result<()> {
    let mut payload = serde_json::to_string_pretty(data)?;
    payload.push('\n');
    let backup = backup_path(file);
    let previous = fs::read_to_string(file).ok().filter(|raw| valid_json(raw));
    match previous {
        Some(prev) => write_file_atomic(&backup, prev.as_bytes(), 0o600)?,
        None => {
            let good = fs::read_to_string(&backup)
                .map(|raw| valid_json(&raw))
                .unwrap_or(false);
            if !good {
                write_file_atomic(&backup, payload.as_bytes(), 0o600)?;
            }
        }
    }
    write_file_atomic(file, payload.as_bytes(), 0o600)
}

/// Like [`write_json_atomic`], but for a save that removes secrets: the old
/// generation is NOT kept. Both the main file and `<file>.bak` receive the new
/// payload (backup first), so no pre-scrub copy survives next to the file.
pub fn write_json_scrubbed<T: Serialize>(file: &Path, data: &T) -> Result<()> {
    let mut payload = serde_json::to_string_pretty(data)?;
    payload.push('\n');
    write_file_atomic(&backup_path(file), payload.as_bytes(), 0o600)?;
    write_file_atomic(file, payload.as_bytes(), 0o600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saves_retain_last_known_good_and_recover_truncated_main() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");
        write_json_atomic(&file, &json!({"profiles": [{"id": "first"}]})).unwrap();
        write_json_atomic(&file, &json!({"profiles": [{"id": "second"}]})).unwrap();
        fs::write(&file, "{\"profiles\":").unwrap();

        let first = json!({"profiles": [{"id": "first"}]});
        assert_eq!(read_json_value(&file), Some(first.clone()));
        let on_disk: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(on_disk, first);
        // The good backup was not overwritten by the corrupt main.
        let bak: Value =
            serde_json::from_str(&fs::read_to_string(backup_path(&file)).unwrap()).unwrap();
        assert_eq!(bak, first);
    }

    #[test]
    fn corrupt_main_is_not_rotated_over_good_backup_on_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.json");
        write_json_atomic(&file, &json!({"v": 1})).unwrap();
        write_json_atomic(&file, &json!({"v": 2})).unwrap();
        fs::write(&file, "{").unwrap();
        write_json_atomic(&file, &json!({"v": 3})).unwrap();
        let bak: Value =
            serde_json::from_str(&fs::read_to_string(backup_path(&file)).unwrap()).unwrap();
        assert_eq!(bak, json!({"v": 1}));
    }

    #[test]
    fn first_save_creates_two_readable_copies() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.json");
        write_json_atomic(&file, &json!({"language": "de"})).unwrap();
        let expect = json!({"language": "de"});
        let main: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let bak: Value =
            serde_json::from_str(&fs::read_to_string(backup_path(&file)).unwrap()).unwrap();
        assert_eq!(main, expect);
        assert_eq!(bak, expect);
    }

    #[test]
    fn format_is_pretty_with_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("c.json");
        write_json_atomic(&file, &json!({"a": 1})).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\n  \"a\": 1\n}\n");
    }

    #[test]
    fn missing_everything_gives_none_and_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("none.json");
        assert_eq!(read_json_value(&file), None);
        assert_eq!(read_json_or(&file, 7u32), 7);
    }

    #[cfg(unix)]
    #[test]
    fn files_are_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.json");
        write_json_atomic(&file, &json!({})).unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn scrubbed_save_leaves_no_old_generation_in_bak() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");
        write_json_atomic(&file, &json!({"t": "PLAINTEXT-SECRET"})).unwrap();
        write_json_atomic(&file, &json!({"t": "PLAINTEXT-SECRET", "x": 1})).unwrap();
        write_json_scrubbed(&file, &json!({"x": 1})).unwrap();
        for f in [file.clone(), backup_path(&file)] {
            let raw = fs::read_to_string(&f).unwrap();
            assert!(!raw.contains("PLAINTEXT-SECRET"), "{f:?}");
            assert!(valid_json(&raw));
        }
    }
}

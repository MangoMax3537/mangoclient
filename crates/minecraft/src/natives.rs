//! Flat extraction of native libraries, guarded by a stamp file.

use crate::error::{MinecraftError, Result};
use crate::libraries::LibraryFile;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const STAMP_FILE: &str = ".mangoclient-stamp";
const MAX_NATIVE_BYTES: u64 = 512 * 1024 * 1024;

fn is_native_ext(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".so", ".dll", ".dylib", ".jnilib"]
        .iter()
        .any(|e| lower.ends_with(e))
}

fn extract_one(archive: &Path, dir: &Path, exclude: &[String]) -> std::result::Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().map_err(|e| e.to_string())?.to_string();
        if name.starts_with("META-INF/") || exclude.iter().any(|ex| name.starts_with(ex.as_str())) {
            continue;
        }
        if !is_native_ext(&name) {
            continue;
        }
        let base = name.rsplit(['/', '\\']).next().unwrap_or("");
        if base.is_empty() || base == "." || base == ".." {
            continue;
        }
        let mut out = File::create(dir.join(base)).map_err(|e| e.to_string())?;
        std::io::copy(&mut (&mut entry).take(MAX_NATIVE_BYTES), &mut out)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Extract `natives` into `natives_root/<version_id>/` once. The directory is
/// wiped and rebuilt whenever the set of archives (name + sha1) changes.
pub async fn extract_natives(
    natives_root: &Path,
    version_id: &str,
    natives: &[LibraryFile],
    on_log: &(dyn Fn(&str) + Send + Sync),
) -> Result<PathBuf> {
    let dir = natives_root.join(version_id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut parts: Vec<String> = natives
        .iter()
        .map(|n| format!("{}:{}", n.name, n.sha1.clone().unwrap_or_default()))
        .collect();
    parts.sort();
    let stamp = parts.join("|");
    let stamp_file = dir.join(STAMP_FILE);
    if tokio::fs::read_to_string(&stamp_file).await.ok().as_deref() == Some(stamp.as_str()) {
        return Ok(dir);
    }
    // A leftover library from an earlier, wrongly selected set would still be
    // found by java.library.path.
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await?;

    let jobs: Vec<(String, PathBuf, Vec<String>)> = natives
        .iter()
        .map(|n| (n.name.clone(), n.path.clone(), n.exclude.clone()))
        .collect();
    let target = dir.clone();
    let failures = tokio::task::spawn_blocking(move || {
        jobs.into_iter()
            .filter_map(|(name, path, exclude)| {
                extract_one(&path, &target, &exclude)
                    .err()
                    .map(|e| format!("Could not extract natives from {name}: {e}"))
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| MinecraftError::Join(e.to_string()))?;
    for line in &failures {
        on_log(line);
    }
    // Only a complete set is marked done: with a failed extraction (an antivirus or another process holding a DLL on
    // Windows) the next launch tries again instead of reusing a broken folder forever.
    if failures.is_empty() {
        tokio::fs::write(&stamp_file, stamp).await?;
    }
    Ok(dir)
}

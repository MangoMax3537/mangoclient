//! Writing a file the user picked in a "save as" dialog: validation of the
//! path and an atomic write (temp file next to it, then rename).

use crate::error::{AppError, Res};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use ts_rs::TS;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Result of the commands that write a file for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileExport {
    pub path: String,
    pub size_bytes: u64,
    /// Things the user should know were left out or changed. Also sent as
    /// `launch:log` warnings of the profile.
    pub warnings: Vec<String>,
}

fn bad_path(why: &str) -> AppError {
    AppError::new("export.invalid_path", why.to_string())
}

/// An absolute path to a new or existing regular file (not a symlink) inside
/// an existing directory, and not inside any of `forbidden` (the instance
/// being read, the launcher data dir, the bundled resources).
pub(crate) fn validate_out_path(raw: &str, forbidden: &[&Path]) -> Res<PathBuf> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') {
        return Err(bad_path("Invalid output path"));
    }
    crate::path_grants::reject_special_path(raw)?;
    let out = PathBuf::from(raw);
    if !out.is_absolute() {
        return Err(bad_path("The output path must be absolute"));
    }
    let (Some(parent), Some(_)) = (out.parent(), out.file_name()) else {
        return Err(bad_path("The output path needs a file name"));
    };
    let parent_meta =
        fs::metadata(parent).map_err(|_| bad_path("The output folder does not exist"))?;
    if !parent_meta.is_dir() {
        return Err(bad_path("The output folder does not exist"));
    }
    if let Ok(meta) = fs::symlink_metadata(&out) {
        if meta.is_dir() {
            return Err(bad_path("The output path is a folder"));
        }
        if meta.file_type().is_symlink() {
            return Err(bad_path("The output path is a symbolic link"));
        }
    }
    if let Ok(real_parent) = fs::canonicalize(parent) {
        for f in forbidden {
            if fs::canonicalize(f).is_ok_and(|f| real_parent.starts_with(f)) {
                return Err(bad_path(
                    "The output file cannot be inside the launcher data or the instance",
                ));
            }
        }
    }
    Ok(out)
}

pub(crate) fn zip_file_options(name: &str, len: u64) -> SimpleFileOptions {
    let lower = name.to_ascii_lowercase();
    let stored = [".jar", ".zip", ".png", ".ogg", ".jpg", ".mca"]
        .iter()
        .any(|e| lower.ends_with(e));
    SimpleFileOptions::default()
        .compression_method(if stored {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        })
        .unix_permissions(0o644)
        .large_file(len >= 0xFFFF_FFFF)
}

fn failed(e: impl std::fmt::Display) -> AppError {
    AppError::new("export.failed", format!("Could not write the file: {e}"))
}

/// Run `fill` on a zip writer over a temp file next to `out`, then move it
/// into place. Nothing is left behind on failure. Returns the final size.
pub(crate) fn write_zip_atomic(
    out: &Path,
    fill: impl FnOnce(&mut ZipWriter<BufWriter<File>>) -> Res<()>,
) -> Res<u64> {
    let parent = out.parent().ok_or_else(|| bad_path("No output folder"))?;
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let tmp = parent.join(format!(".{name}.part-{}-{nonce}", std::process::id()));
    let result = (|| -> Res<u64> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(failed)?;
        let mut zw = ZipWriter::new(BufWriter::new(file));
        fill(&mut zw)?;
        let mut w = zw.finish().map_err(failed)?;
        w.flush().map_err(failed)?;
        let file = w.into_inner().map_err(|e| failed(e.error()))?;
        file.sync_all().map_err(failed)?;
        let size = file.metadata().map_err(failed)?.len();
        drop(file);
        fs::rename(&tmp, out).map_err(failed)?;
        Ok(size)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Add a file entry with `bytes`.
pub(crate) fn add_bytes<W: Write + std::io::Seek>(
    zw: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
) -> Res<()> {
    zw.start_file(name, zip_file_options(name, bytes.len() as u64))
        .map_err(failed)?;
    zw.write_all(bytes).map_err(failed)?;
    Ok(())
}

/// Add a file entry streamed from `src`, at most `max` bytes.
pub(crate) fn add_file<W: Write + std::io::Seek>(
    zw: &mut ZipWriter<W>,
    name: &str,
    src: &Path,
    len: u64,
) -> Res<()> {
    use std::io::Read;
    zw.start_file(name, zip_file_options(name, len))
        .map_err(failed)?;
    let file = File::open(src).map_err(failed)?;
    let n = std::io::copy(&mut file.take(len), zw).map_err(failed)?;
    if n != len {
        return Err(failed("a file changed while it was being read"));
    }
    Ok(())
}

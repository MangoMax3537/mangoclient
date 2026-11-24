//! Safe archive path resolution and extraction (zip, tar.gz).
//!
//! Port of `src/main/archive.js`: paths may not be absolute, contain NUL,
//! escape the root, or pass through an existing symlink; archives are bounded
//! by entry count, per-entry size and total size, and symlink entries are
//! rejected.

use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

/// Unix file-type mask and symlink value as stored in zip external attributes.
const S_IFMT: u32 = 0o170_000;
const S_IFLNK: u32 = 0o120_000;

/// Extraction limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of entries in an archive.
    pub entries: usize,
    /// Maximum uncompressed size of one entry, in bytes.
    pub entry_size: u64,
    /// Maximum total uncompressed size, in bytes.
    pub total_size: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 50_000,
            entry_size: 256 * 1024 * 1024,
            total_size: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("Archive contains an invalid path")]
    InvalidPath,
    #[error("Archive path escapes its instance: {0}")]
    Escapes(String),
    #[error("Archive path uses a symlink: {0}")]
    Symlink(String),
    #[error("Archive contains a symlink: {0}")]
    SymlinkEntry(String),
    #[error("Archive contains an unsupported entry: {0}")]
    UnsupportedEntry(String),
    #[error("Archive has too many entries")]
    TooManyEntries,
    #[error("Archive entry is too large: {0}")]
    EntryTooLarge(String),
    #[error("Archive overrides are too large")]
    TotalTooLarge,
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Minimal description of an archive entry for limit validation.
#[derive(Debug, Clone)]
pub struct EntryInfo {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    /// Unix mode (zip external attributes >> 16), if known.
    pub unix_mode: Option<u32>,
}

/// True when the unix mode marks a symlink.
pub fn is_symlink_mode(mode: Option<u32>) -> bool {
    mode.is_some_and(|m| m & S_IFMT == S_IFLNK)
}

/// True when any path component of an archive entry starts with
/// `.mangoclient-` (case-insensitive), at any depth: the launcher's own
/// bookkeeping files (for example `mods/.mangoclient-performance.json`) are
/// never taken from a pack, whatever the folder or letter case.
pub fn is_launcher_private(rel: &str) -> bool {
    const PREFIX: &[u8] = b".mangoclient-";
    rel.split(['/', '\\']).any(|part| {
        part.as_bytes()
            .get(..PREFIX.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(PREFIX))
    })
}

/// Resolve `relative` under `root`, rejecting absolute paths, NUL, escapes via
/// `..` and any existing symlink along the way. Does not touch the disk beyond
/// `symlink_metadata` lookups; non-existent components are fine.
pub fn safe_archive_path(root: &Path, relative: &str) -> Result<PathBuf, ArchiveError> {
    if relative.contains('\0') || Path::new(relative).is_absolute() || relative.starts_with('/') {
        return Err(ArchiveError::InvalidPath);
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in Path::new(relative).components() {
        match comp {
            Component::Normal(p) => parts.push(p),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(ArchiveError::Escapes(relative.to_string()));
                }
            }
            Component::RootDir | Component::Prefix(_) => return Err(ArchiveError::InvalidPath),
        }
    }
    let mut current = root.to_path_buf();
    let mut linked = false;
    for part in parts {
        current.push(part);
        if linked {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(ArchiveError::Symlink(relative.to_string()));
            }
            Ok(_) => {}
            // Nothing exists from here on, so nothing further can be a link.
            Err(e) if e.kind() == io::ErrorKind::NotFound => linked = true,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(current)
}

/// Validate entry count, per-entry size, total size and symlinks.
/// Returns the total uncompressed size of non-directory entries.
pub fn validate_entries<I>(entries: I, limits: &Limits) -> Result<u64, ArchiveError>
where
    I: IntoIterator<Item = EntryInfo>,
{
    let mut count = 0usize;
    let mut total = 0u64;
    for entry in entries {
        count += 1;
        if count > limits.entries {
            return Err(ArchiveError::TooManyEntries);
        }
        if is_symlink_mode(entry.unix_mode) {
            return Err(ArchiveError::SymlinkEntry(entry.name));
        }
        if entry.is_dir {
            continue;
        }
        if entry.size > limits.entry_size {
            return Err(ArchiveError::EntryTooLarge(entry.name));
        }
        total = total.saturating_add(entry.size);
        if total > limits.total_size {
            return Err(ArchiveError::TotalTooLarge);
        }
    }
    Ok(total)
}

/// Copy at most `max` bytes; error if the stream holds more (lying headers).
fn copy_bounded<R: Read>(
    src: &mut R,
    dst: &mut impl Write,
    max: u64,
    name: &str,
) -> Result<u64, ArchiveError> {
    let n = io::copy(&mut src.take(max.saturating_add(1)), dst)?;
    if n > max {
        return Err(ArchiveError::EntryTooLarge(name.to_string()));
    }
    Ok(n)
}

/// Extract a zip archive into `dest` (created if needed). Returns bytes written.
pub fn extract_zip<R: Read + Seek>(
    reader: R,
    dest: &Path,
    limits: &Limits,
) -> Result<u64, ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader)?;
    if zip.len() > limits.entries {
        return Err(ArchiveError::TooManyEntries);
    }
    let mut infos = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i)?;
        infos.push(EntryInfo {
            name: f.name()?.to_string(),
            size: f.size(),
            is_dir: f.is_dir(),
            unix_mode: f.unix_mode(),
        });
    }
    validate_entries(infos, limits)?;

    fs::create_dir_all(dest)?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f.name()?.to_string();
        let target = safe_archive_path(dest, &name)?;
        if f.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            // Re-check after creating directories (existing links are refused).
            safe_archive_path(dest, &name)?;
        }
        let mut out = fs::File::create(&target)?;
        let remaining = limits.total_size.saturating_sub(total);
        let n = copy_bounded(&mut f, &mut out, limits.entry_size.min(remaining), &name)?;
        total += n;
    }
    Ok(total)
}

/// The entries of `zip` below `prefix` (`""` = all, else with a trailing
/// `/`) that `skip` does not exclude, as `(index, path relative to the
/// prefix, info)`. Entries named exactly like the prefix are ignored.
fn subtree_entries<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    prefix: &str,
    skip: &dyn Fn(&str) -> bool,
) -> Result<Vec<(usize, String, EntryInfo)>, ArchiveError> {
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i)?;
        let name = f.name()?.to_string();
        let Some(rel) = name.strip_prefix(prefix) else {
            continue;
        };
        if rel.is_empty() || skip(rel) {
            continue;
        }
        let info = EntryInfo {
            name: name.clone(),
            size: f.size(),
            is_dir: f.is_dir(),
            unix_mode: f.unix_mode(),
        };
        out.push((i, rel.to_string(), info));
    }
    Ok(out)
}

/// Check the part of a zip below `prefix` without writing anything: entry
/// count (of the whole archive), symlink entries, sizes and that every path
/// stays inside `check_root`. Returns `(files, bytes)` of what an extraction
/// would write.
pub fn scan_zip_subtree<R: Read + Seek>(
    reader: R,
    prefix: &str,
    check_root: &Path,
    limits: &Limits,
    skip: &dyn Fn(&str) -> bool,
) -> Result<(usize, u64), ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader)?;
    if zip.len() > limits.entries {
        return Err(ArchiveError::TooManyEntries);
    }
    let entries = subtree_entries(&mut zip, prefix, skip)?;
    let bytes = validate_entries(entries.iter().map(|(_, _, e)| e.clone()), limits)?;
    let mut files = 0;
    for (_, rel, info) in &entries {
        safe_archive_path(check_root, rel)?;
        if !info.is_dir {
            files += 1;
        }
    }
    Ok((files, bytes))
}

/// Extract the part of a zip below `prefix` into `dest` (created if needed),
/// leaving out what `skip` (given the path relative to the prefix) excludes.
/// Same guarantees as [`extract_zip`]: no traversal, no links followed or
/// created, bounded entry count, sizes and total. An existing regular file is
/// overwritten, an existing link or directory in the way is an error.
pub fn extract_zip_subtree<R: Read + Seek>(
    reader: R,
    prefix: &str,
    dest: &Path,
    limits: &Limits,
    skip: &dyn Fn(&str) -> bool,
) -> Result<u64, ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader)?;
    if zip.len() > limits.entries {
        return Err(ArchiveError::TooManyEntries);
    }
    let entries = subtree_entries(&mut zip, prefix, skip)?;
    validate_entries(entries.iter().map(|(_, _, e)| e.clone()), limits)?;
    for (_, rel, _) in &entries {
        safe_archive_path(dest, rel)?;
    }
    fs::create_dir_all(dest)?;
    let mut total = 0u64;
    for (i, rel, info) in entries {
        let target = safe_archive_path(dest, &rel)?;
        if info.is_dir {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            // Re-check after creating directories (existing links are refused).
            safe_archive_path(dest, &rel)?;
        }
        let mut out = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(o) => o,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&target)?;
                if meta.file_type().is_symlink() || !meta.is_file() {
                    return Err(ArchiveError::Symlink(rel));
                }
                fs::File::create(&target)?
            }
            Err(e) => return Err(e.into()),
        };
        let mut f = zip.by_index(i)?;
        let remaining = limits.total_size.saturating_sub(total);
        total += copy_bounded(
            &mut f,
            &mut out,
            limits.entry_size.min(remaining),
            &info.name,
        )?;
    }
    Ok(total)
}

/// Extract a `.tar.gz` stream into `dest` (created if needed). Returns bytes written.
pub fn extract_tar_gz<R: Read>(
    reader: R,
    dest: &Path,
    limits: &Limits,
) -> Result<u64, ArchiveError> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    fs::create_dir_all(dest)?;
    let mut count = 0usize;
    let mut total = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        count += 1;
        if count > limits.entries {
            return Err(ArchiveError::TooManyEntries);
        }
        let name = entry.path()?.to_string_lossy().into_owned();
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(ArchiveError::SymlinkEntry(name));
        }
        let target = safe_archive_path(dest, &name)?;
        if kind.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if !kind.is_file() {
            return Err(ArchiveError::UnsupportedEntry(name));
        }
        if entry.size() > limits.entry_size {
            return Err(ArchiveError::EntryTooLarge(name));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            safe_archive_path(dest, &name)?;
        }
        let mode = entry.header().mode().ok();
        let mut out = fs::File::create(&target)?;
        let remaining = limits.total_size.saturating_sub(total);
        let n = copy_bounded(
            &mut entry,
            &mut out,
            limits.entry_size.min(remaining),
            &name,
        )?;
        total += n;
        keep_exec_bit(&out, mode)?;
    }
    Ok(total)
}

/// Executables keep their execute bit (a JRE's `bin/*` and `lib/jspawnhelper`); nothing else is taken from the
/// archive: the result is 0755 or the default mode, never setuid/setgid/sticky or world-writable.
#[cfg(unix)]
fn keep_exec_bit(file: &fs::File, mode: Option<u32>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if mode.is_some_and(|m| m & 0o111 != 0) {
        file.set_permissions(fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn keep_exec_bit(_file: &fs::File, _mode: Option<u32>) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod private_name_tests {
    use super::is_launcher_private;

    #[test]
    fn launcher_bookkeeping_is_recognised_at_any_depth_and_case() {
        for p in [
            ".mangoclient-x",
            "mods/.mangoclient-performance.json",
            "MODS/.MANGOCLIENT-x",
            "a/b/c/.MangoClient-state",
            ".mangoclient-dir/inside.txt",
            "mods\\.mangoclient-x",
        ] {
            assert!(is_launcher_private(p), "{p}");
        }
        for p in [
            "mods/a.jar",
            "config/mangoclient-x.toml",
            "x.mangoclient-y",
            ".mangoclient",
            "",
        ] {
            assert!(!is_launcher_private(p), "{p}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn info(name: &str, size: u64, mode: Option<u32>) -> EntryInfo {
        EntryInfo {
            name: name.into(),
            size,
            is_dir: false,
            unix_mode: mode,
        }
    }

    #[test]
    fn path_basic_and_traversal() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            safe_archive_path(root.path(), "mods/safe.jar").unwrap(),
            root.path().join("mods").join("safe.jar")
        );
        assert!(matches!(
            safe_archive_path(root.path(), "../escape.jar"),
            Err(ArchiveError::Escapes(_))
        ));
        assert!(matches!(
            safe_archive_path(root.path(), "a/../../x"),
            Err(ArchiveError::Escapes(_))
        ));
        assert!(safe_archive_path(root.path(), "a/../b").is_ok());
        assert!(matches!(
            safe_archive_path(root.path(), "/etc/passwd"),
            Err(ArchiveError::InvalidPath)
        ));
        assert!(matches!(
            safe_archive_path(root.path(), "a\0b"),
            Err(ArchiveError::InvalidPath)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn path_rejects_existing_symlink() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
        assert!(matches!(
            safe_archive_path(root.path(), "linked/escape.jar"),
            Err(ArchiveError::Symlink(_))
        ));
        assert!(matches!(
            safe_archive_path(root.path(), "linked"),
            Err(ArchiveError::Symlink(_))
        ));
    }

    #[test]
    fn limits_cap_entries_sizes_and_symlinks() {
        let l = Limits::default();
        assert_eq!(
            validate_entries([info("a", 10, None), info("b", 20, None)], &l).unwrap(),
            30
        );
        let many = (0..=l.entries).map(|i| info(&i.to_string(), 0, None));
        assert!(matches!(
            validate_entries(many, &l),
            Err(ArchiveError::TooManyEntries)
        ));
        assert!(matches!(
            validate_entries([info("huge", l.entry_size + 1, None)], &l),
            Err(ArchiveError::EntryTooLarge(_))
        ));
        assert!(matches!(
            validate_entries([info("link", 1, Some(0o120_777))], &l),
            Err(ArchiveError::SymlinkEntry(_))
        ));
        let nine = (0..9).map(|i| info(&i.to_string(), l.entry_size, None));
        assert!(matches!(
            validate_entries(nine, &l),
            Err(ArchiveError::TotalTooLarge)
        ));
    }

    fn make_zip(entries: &[(&str, &[u8], Option<u32>)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, mode) in entries {
            let opts = zip::write::SimpleFileOptions::default();
            if mode.is_some() {
                w.add_symlink(*name, String::from_utf8_lossy(data), opts)
                    .unwrap();
            } else {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn zip_extracts_and_rejects_bad_entries() {
        let dest = tempfile::tempdir().unwrap();
        let ok = make_zip(&[("mods/a.jar", b"hello", None), ("b.txt", b"x", None)]);
        let n = extract_zip(Cursor::new(ok), dest.path(), &Limits::default()).unwrap();
        assert_eq!(n, 6);
        assert_eq!(fs::read(dest.path().join("mods/a.jar")).unwrap(), b"hello");

        let evil = make_zip(&[("../evil.txt", b"x", None)]);
        assert!(extract_zip(Cursor::new(evil), dest.path(), &Limits::default()).is_err());

        let link = make_zip(&[("l", b"/etc", Some(0o120_777))]);
        assert!(matches!(
            extract_zip(Cursor::new(link), dest.path(), &Limits::default()),
            Err(ArchiveError::SymlinkEntry(_))
        ));

        let tiny = Limits {
            entry_size: 3,
            ..Limits::default()
        };
        let big = make_zip(&[("big", b"12345", None)]);
        assert!(matches!(
            extract_zip(Cursor::new(big), dest.path(), &tiny),
            Err(ArchiveError::EntryTooLarge(_))
        ));
    }

    #[test]
    fn subtree_scan_and_extract_filter_validate_and_never_escape() {
        let zip = make_zip(&[
            ("pack/.minecraft/mods/a.jar", b"jar", None),
            ("pack/.minecraft/options.txt", b"lang:de", None),
            ("pack/.minecraft/logs/latest.log", b"noise", None),
            ("pack/instance.cfg", b"name=x", None),
        ]);
        let skip = |rel: &str| rel.starts_with("logs/");
        let root = tempfile::tempdir().unwrap();
        let (files, bytes) = scan_zip_subtree(
            Cursor::new(zip.clone()),
            "pack/.minecraft/",
            root.path(),
            &Limits::default(),
            &skip,
        )
        .unwrap();
        assert_eq!((files, bytes), (2, 10));
        let dest = root.path().join("out");
        let n = extract_zip_subtree(
            Cursor::new(zip),
            "pack/.minecraft/",
            &dest,
            &Limits::default(),
            &skip,
        )
        .unwrap();
        assert_eq!(n, 10);
        assert_eq!(fs::read(dest.join("mods/a.jar")).unwrap(), b"jar");
        assert!(!dest.join("logs").exists() && !dest.join("instance.cfg").exists());

        let evil = make_zip(&[("m/../../escape.txt", b"x", None), ("m/ok", b"x", None)]);
        let out = root.path().join("evil");
        assert!(extract_zip_subtree(
            Cursor::new(evil.clone()),
            "m/",
            &out,
            &Limits::default(),
            &|_| false
        )
        .is_err());
        assert!(!out.exists(), "nothing is written before validation");
        assert!(scan_zip_subtree(
            Cursor::new(evil),
            "m/",
            root.path(),
            &Limits::default(),
            &|_| false
        )
        .is_err());
        let link = make_zip(&[("m/l", b"/etc", Some(0o120_777))]);
        assert!(matches!(
            scan_zip_subtree(
                Cursor::new(link),
                "m/",
                root.path(),
                &Limits::default(),
                &|_| false
            ),
            Err(ArchiveError::SymlinkEntry(_))
        ));
    }

    fn make_tar_gz(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        build(&mut b);
        let raw = b.into_inner().unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn tar_gz_keeps_the_execute_bit_but_nothing_dangerous() {
        use std::os::unix::fs::PermissionsExt;
        let dest = tempfile::tempdir().unwrap();
        let gz = make_tar_gz(|b| {
            for (name, mode) in [
                ("jre/lib/jspawnhelper", 0o755),
                ("jre/lib/modules", 0o644),
                ("jre/bin/evil", 0o4777),
            ] {
                let mut h = tar::Header::new_gnu();
                h.set_size(1);
                h.set_mode(mode);
                h.set_cksum();
                b.append_data(&mut h, name, &b"x"[..]).unwrap();
            }
        });
        extract_tar_gz(&gz[..], dest.path(), &Limits::default()).unwrap();
        let mode = |p: &str| {
            fs::metadata(dest.path().join(p))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        assert_eq!(mode("jre/lib/jspawnhelper"), 0o755);
        assert_eq!(
            mode("jre/lib/modules") & 0o111,
            0,
            "data files stay non-executable"
        );
        assert_eq!(
            mode("jre/bin/evil"),
            0o755,
            "setuid and world-writable bits are dropped"
        );
    }

    #[test]
    fn tar_gz_extracts_and_rejects_links() {
        let dest = tempfile::tempdir().unwrap();
        let good = make_tar_gz(|b| {
            let mut h = tar::Header::new_gnu();
            h.set_size(2);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, "dir/f.txt", &b"hi"[..]).unwrap();
        });
        let n = extract_tar_gz(&good[..], dest.path(), &Limits::default()).unwrap();
        assert_eq!(n, 2);
        assert_eq!(fs::read(dest.path().join("dir/f.txt")).unwrap(), b"hi");

        let link = make_tar_gz(|b| {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_size(0);
            h.set_cksum();
            b.append_link(&mut h, "l", "/etc").unwrap();
        });
        assert!(matches!(
            extract_tar_gz(&link[..], dest.path(), &Limits::default()),
            Err(ArchiveError::SymlinkEntry(_))
        ));

        let trav = make_tar_gz(|b| {
            let mut h = tar::Header::new_gnu();
            h.set_size(1);
            h.set_mode(0o644);
            h.set_cksum();
            // set_path refuses "..", so write the name bytes directly.
            let raw = h.as_old_mut();
            raw.name[..6].copy_from_slice(b"../bad");
            h.set_cksum();
            b.append(&h, &b"x"[..]).unwrap();
        });
        assert!(matches!(
            extract_tar_gz(&trav[..], dest.path(), &Limits::default()),
            Err(ArchiveError::Escapes(_))
        ));
    }
}

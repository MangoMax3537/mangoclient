//! Real [`Installer`] for the three desktop platforms.
//!
//! All decision logic (path checks, backup, rollback, "only the verified file")
//! is platform independent and compiled everywhere, so it is tested on any OS.
//! Process spawning goes through the [`Spawner`] trait; the shell supplies the
//! quit callback. The staged path is never taken from outside: it is the file
//! the updater just downloaded and verified, and it must sit inside the updater's
//! download directory.
//!
//! - Linux AppImage: copy next to `$APPIMAGE` as `.<name>.new` (mode 0755), on
//!   apply hard-link the current image to `<name>.old`, then atomically rename the
//!   new file over it. The running image is never missing; a failure leaves it intact.
//! - Windows NSIS: the verified per-user setup is spawned detached with `/S /UPDATE`
//!   (silent, Tauri's NSIS template relaunches the app in update mode), then the app quits.
//! - macOS: the zip is extracted with the safe extractor into a hidden directory
//!   beside the bundle; apply swaps the bundle with a rename and a hidden backup,
//!   rolling back if the second rename fails, then relaunches via `open -n`.

use crate::error::{Result, UpdateError};
use crate::installer::Installer;
use mango_archive::{extract_zip, Limits};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Starts processes detached from the launcher.
pub trait Spawner: Send + Sync {
    fn spawn_detached(&self, program: &Path, args: &[OsString]) -> io::Result<()>;
}

/// Called after a successful hand-over so the shell can exit.
pub type QuitFn = Arc<dyn Fn() + Send + Sync>;

/// [`Spawner`] backed by `std::process::Command` with null stdio.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessSpawner;

impl Spawner for ProcessSpawner {
    fn spawn_detached(&self, program: &Path, args: &[OsString]) -> io::Result<()> {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
            cmd.creation_flags(0x0000_0008 | 0x0000_0200);
        }
        cmd.spawn().map(|_| ())
    }
}

/// What this install is and what it replaces.
#[derive(Debug, Clone)]
pub enum Target {
    /// Absolute path of the running AppImage (`$APPIMAGE`).
    AppImage(PathBuf),
    /// Windows per-user NSIS install.
    WindowsNsis,
    /// Absolute path of the running `.app` bundle.
    MacApp(PathBuf),
}

#[derive(Debug, Clone)]
enum Staged {
    /// A single file with its SHA-256 recorded at stage time.
    File { path: PathBuf, sha256: [u8; 32] },
    /// An unpacked `.app` directory.
    Bundle { path: PathBuf },
}

pub struct PlatformInstaller {
    target: Target,
    /// The updater's download directory; the only place a verified file may come from.
    allowed_dir: PathBuf,
    spawner: Arc<dyn Spawner>,
    quit: QuitFn,
    staged: Mutex<Option<Staged>>,
}

fn fail(msg: impl Into<String>) -> UpdateError {
    UpdateError::Install(msg.into())
}

fn sha256_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

/// `file` must be a regular, non-symlink file whose real path is inside `dir`.
fn verified_source(file: &Path, dir: &Path) -> Result<PathBuf> {
    let meta = fs::symlink_metadata(file)?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(fail("staged update is not a regular file"));
    }
    let real = fs::canonicalize(file)?;
    let real_dir = fs::canonicalize(dir)?;
    if !real.starts_with(&real_dir) {
        return Err(fail("staged update is outside the download directory"));
    }
    Ok(real)
}

/// The path being replaced must be a regular entry (not a link) whose parent is a
/// real directory, so a swapped-in link cannot redirect the replacement.
fn checked_target(path: &Path, want_dir: bool) -> Result<(PathBuf, OsString)> {
    let name = path
        .file_name()
        .ok_or_else(|| fail("install target has no file name"))?
        .to_os_string();
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| fail("install target has no parent directory"))?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(fail("install target is a symlink"));
    }
    if want_dir != meta.is_dir() || (!want_dir && !meta.is_file()) {
        return Err(fail("install target has the wrong type"));
    }
    let real_parent = fs::canonicalize(parent)?;
    if parent != real_parent || fs::canonicalize(path)? != real_parent.join(&name) {
        return Err(fail("install target path goes through a symlink"));
    }
    Ok((real_parent, name))
}

fn sibling(dir: &Path, prefix: &str, name: &OsStr, suffix: &str) -> PathBuf {
    let mut n = OsString::from(prefix);
    n.push(name);
    n.push(suffix);
    dir.join(n)
}

fn remove_any(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Copy `src` to a fresh `dest` (never overwrites through a link) and mark it executable.
fn copy_executable(src: &Path, dest: &Path) -> io::Result<()> {
    remove_any(dest)?;
    let mut input = fs::File::open(src)?;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o755);
    }
    let mut out = opts.open(dest)?;
    io::copy(&mut input, &mut out)?;
    out.sync_all()?;
    drop(out);
    set_mode(dest, 0o755)
}

impl PlatformInstaller {
    pub fn new(
        target: Target,
        allowed_dir: impl Into<PathBuf>,
        spawner: Arc<dyn Spawner>,
        quit: QuitFn,
    ) -> Self {
        Self {
            target,
            allowed_dir: allowed_dir.into(),
            spawner,
            quit,
            staged: Mutex::new(None),
        }
    }

    fn staged(&self) -> MutexGuard<'_, Option<Staged>> {
        self.staged.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn stage_appimage(&self, current: &Path, file: &Path) -> Result<Staged> {
        let src = verified_source(file, &self.allowed_dir)?;
        let (dir, name) = checked_target(current, false)?;
        let dest = sibling(&dir, ".", &name, ".new");
        copy_executable(&src, &dest)?;
        let sha256 = sha256_file(&dest)?;
        if sha256 != sha256_file(&src)? {
            let _ = fs::remove_file(&dest);
            return Err(fail("staged copy does not match the verified download"));
        }
        Ok(Staged::File { path: dest, sha256 })
    }

    fn apply_appimage(&self, current: &Path, staged: &Staged) -> Result<()> {
        let Staged::File { path, sha256 } = staged else {
            return Err(fail("unexpected staged kind"));
        };
        let (dir, name) = checked_target(current, false)?;
        if *path != sibling(&dir, ".", &name, ".new") {
            return Err(fail("staged file is not the one beside the AppImage"));
        }
        recheck_file(path, sha256)?;
        let live = dir.join(&name);
        let old = sibling(&dir, "", &name, ".old");
        remove_any(&old)?;
        // A hard link keeps the live image in place; copy where links are unsupported.
        if fs::hard_link(&live, &old).is_err() {
            fs::copy(&live, &old)?;
        }
        if let Err(e) = fs::rename(path, &live) {
            let _ = fs::remove_file(&old);
            return Err(e.into());
        }
        Ok(())
    }

    fn stage_windows(&self, file: &Path) -> Result<Staged> {
        let src = verified_source(file, &self.allowed_dir)?;
        let is_exe = src
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        if !is_exe {
            return Err(fail("Windows update is not a setup executable"));
        }
        Ok(Staged::File {
            sha256: sha256_file(&src)?,
            path: src,
        })
    }

    fn stage_mac(&self, bundle: &Path, file: &Path) -> Result<Staged> {
        let src = verified_source(file, &self.allowed_dir)?;
        let (dir, name) = checked_target(bundle, true)?;
        let work = sibling(&dir, ".", &name, ".update");
        remove_any(&work)?;
        let result = (|| -> Result<PathBuf> {
            let reader = fs::File::open(&src)?;
            extract_zip(reader, &work, &Limits::default())
                .map_err(|e| fail(format!("could not unpack update: {e}")))?;
            let app = single_app_dir(&work)?;
            // The extractor does not restore modes; the launcher binary must run.
            make_bundle_executables(&app)?;
            Ok(app)
        })();
        match result {
            Ok(path) => Ok(Staged::Bundle { path }),
            Err(e) => {
                let _ = remove_any(&work);
                Err(e)
            }
        }
    }

    fn apply_mac(&self, bundle: &Path, staged: &Staged) -> Result<()> {
        let Staged::Bundle { path } = staged else {
            return Err(fail("unexpected staged kind"));
        };
        let (dir, name) = checked_target(bundle, true)?;
        let work = sibling(&dir, ".", &name, ".update");
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink() || !meta.is_dir() || !path.starts_with(&work) {
            return Err(fail("staged bundle is not the unpacked update"));
        }
        let live = dir.join(&name);
        let backup = sibling(&dir, ".", &name, ".old");
        remove_any(&backup)?;
        fs::rename(&live, &backup)?;
        if let Err(e) = fs::rename(path, &live) {
            // Roll back so the launcher is never left without an app.
            let _ = fs::rename(&backup, &live);
            return Err(e.into());
        }
        let _ = remove_any(&work);
        Ok(())
    }

    fn relaunch_args(&self) -> Option<(PathBuf, Vec<OsString>)> {
        match &self.target {
            Target::AppImage(p) => Some((p.clone(), Vec::new())),
            Target::MacApp(p) => Some((
                PathBuf::from("open"),
                vec!["-n".into(), p.clone().into_os_string()],
            )),
            Target::WindowsNsis => None,
        }
    }

    /// Delete the `.old` backup left by a previous update (call at startup once the
    /// new version has run). Never fails the caller.
    pub fn cleanup_backup(&self) {
        let (path, hidden) = match &self.target {
            Target::AppImage(p) => (p, false),
            Target::MacApp(p) => (p, true),
            Target::WindowsNsis => return,
        };
        if let (Some(dir), Some(name)) = (path.parent(), path.file_name()) {
            let _ = remove_any(&sibling(dir, if hidden { "." } else { "" }, name, ".old"));
        }
    }
}

fn recheck_file(path: &Path, sha256: &[u8; 32]) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(fail("staged update changed since verification"));
    }
    if sha256_file(path)? != *sha256 {
        return Err(fail("staged update changed since verification"));
    }
    Ok(())
}

/// The unpacked zip must hold exactly one top-level `*.app` directory.
fn single_app_dir(work: &Path) -> Result<PathBuf> {
    let mut apps = Vec::new();
    for entry in fs::read_dir(work)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("__MACOSX") {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        let is_app = name.to_string_lossy().ends_with(".app") && meta.is_dir();
        if !is_app {
            return Err(fail("update archive has unexpected top-level entries"));
        }
        apps.push(entry.path());
    }
    match apps.as_slice() {
        [one] if one.join("Contents").is_dir() => Ok(one.clone()),
        _ => Err(fail("update archive must contain exactly one .app bundle")),
    }
}

fn make_bundle_executables(app: &Path) -> Result<()> {
    let macos = app.join("Contents/MacOS");
    let entries = fs::read_dir(&macos).map_err(|_| fail("update bundle has no Contents/MacOS"))?;
    let mut any = false;
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            set_mode(&entry.path(), 0o755)?;
            any = true;
        }
    }
    if any {
        Ok(())
    } else {
        Err(fail("update bundle has no executable"))
    }
}

impl Installer for PlatformInstaller {
    fn stage(&self, file: &Path) -> Result<()> {
        let staged = match &self.target {
            Target::AppImage(current) => self.stage_appimage(current, file)?,
            Target::WindowsNsis => self.stage_windows(file)?,
            Target::MacApp(bundle) => self.stage_mac(bundle, file)?,
        };
        *self.staged() = Some(staged);
        Ok(())
    }

    fn apply_on_quit(&self) -> Result<()> {
        let staged = self
            .staged()
            .clone()
            .ok_or_else(|| fail("no update has been staged"))?;
        let spawn = match (&self.target, &staged) {
            (Target::AppImage(current), _) => {
                self.apply_appimage(current, &staged)?;
                self.relaunch_args()
            }
            (Target::MacApp(bundle), _) => {
                self.apply_mac(bundle, &staged)?;
                self.relaunch_args()
            }
            (Target::WindowsNsis, Staged::File { path, sha256 }) => {
                recheck_file(path, sha256)?;
                Some((path.clone(), vec!["/S".into(), "/UPDATE".into()]))
            }
            (Target::WindowsNsis, Staged::Bundle { .. }) => {
                return Err(fail("unexpected staged kind"));
            }
        };
        *self.staged() = None;
        if let Some((program, args)) = spawn {
            self.spawner.spawn_detached(&program, &args)?;
        }
        (self.quit)();
        Ok(())
    }
}

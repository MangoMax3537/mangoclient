#![allow(clippy::unwrap_used)]
use crate::platform_install::*;
use crate::Installer;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Rec {
    calls: Mutex<Vec<(PathBuf, Vec<OsString>)>>,
}
impl Spawner for Rec {
    fn spawn_detached(&self, program: &Path, args: &[OsString]) -> std::io::Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push((program.to_path_buf(), args.to_vec()));
        Ok(())
    }
}

struct Fixture {
    _tmp: tempfile::TempDir,
    downloads: PathBuf,
    install: PathBuf,
    rec: Arc<Rec>,
    quits: Arc<AtomicUsize>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let downloads = tmp.path().join("updates");
    let install = tmp.path().join("apps");
    fs::create_dir_all(&downloads).unwrap();
    fs::create_dir_all(&install).unwrap();
    Fixture {
        downloads,
        install,
        _tmp: tmp,
        rec: Arc::new(Rec::default()),
        quits: Arc::new(AtomicUsize::new(0)),
    }
}

impl Fixture {
    fn installer(&self, target: Target) -> PlatformInstaller {
        let q = self.quits.clone();
        PlatformInstaller::new(
            target,
            &self.downloads,
            self.rec.clone(),
            Arc::new(move || {
                q.fetch_add(1, Ordering::SeqCst);
            }),
        )
    }
    fn download(&self, name: &str, body: &[u8]) -> PathBuf {
        let p = self.downloads.join(name);
        fs::write(&p, body).unwrap();
        p
    }
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn appimage_swaps_atomically_keeps_backup_and_relaunches() {
    let f = fixture();
    let live = f.install.join("MangoClient.AppImage");
    fs::write(&live, b"old build").unwrap();
    let new = f.download("new.AppImage", b"new build");
    let inst = f.installer(Target::AppImage(live.clone()));
    inst.stage(&new).unwrap();
    // Staged beside the live image, executable, live untouched.
    let staged = f.install.join(".MangoClient.AppImage.new");
    assert_eq!(fs::read(&staged).unwrap(), b"new build");
    #[cfg(unix)]
    assert_eq!(mode(&staged), 0o755);
    assert_eq!(fs::read(&live).unwrap(), b"old build");

    inst.apply_on_quit().unwrap();
    assert_eq!(fs::read(&live).unwrap(), b"new build");
    assert_eq!(
        fs::read(f.install.join("MangoClient.AppImage.old")).unwrap(),
        b"old build"
    );
    assert!(!staged.exists());
    assert_eq!(f.rec.calls.lock().unwrap()[0], (live, vec![]));
    assert_eq!(f.quits.load(Ordering::SeqCst), 1);
    // Applying twice is refused (nothing staged any more).
    assert!(inst.apply_on_quit().is_err());

    inst.cleanup_backup();
    assert!(!f.install.join("MangoClient.AppImage.old").exists());
}

#[test]
fn apply_without_stage_is_refused() {
    let f = fixture();
    let live = f.install.join("a.AppImage");
    fs::write(&live, b"x").unwrap();
    let inst = f.installer(Target::AppImage(live));
    assert!(inst.apply_on_quit().is_err());
    assert_eq!(f.quits.load(Ordering::SeqCst), 0);
}

#[test]
fn file_outside_download_dir_is_refused() {
    let f = fixture();
    let live = f.install.join("a.AppImage");
    fs::write(&live, b"x").unwrap();
    let evil = f.install.join("evil.AppImage");
    fs::write(&evil, b"evil").unwrap();
    let inst = f.installer(Target::AppImage(live.clone()));
    assert!(inst.stage(&evil).is_err());
    // `..` escape resolved by canonicalization.
    let sneaky = f.downloads.join("../apps/evil.AppImage");
    assert!(inst.stage(&sneaky).is_err());
    assert_eq!(fs::read(&live).unwrap(), b"x");
}

#[cfg(unix)]
#[test]
fn symlinks_are_never_followed() {
    use std::os::unix::fs::symlink;
    let f = fixture();
    let live = f.install.join("a.AppImage");
    fs::write(&live, b"x").unwrap();
    // Download that is a link to a file elsewhere.
    let outside = f.install.join("secret");
    fs::write(&outside, b"s").unwrap();
    let link = f.downloads.join("dl.AppImage");
    symlink(&outside, &link).unwrap();
    let inst = f.installer(Target::AppImage(live.clone()));
    assert!(inst.stage(&link).is_err());

    // Install target that is a link.
    let ok = f.download("ok.AppImage", b"new");
    let target_link = f.install.join("link.AppImage");
    symlink(&live, &target_link).unwrap();
    let inst = f.installer(Target::AppImage(target_link));
    assert!(inst.stage(&ok).is_err());

    // Install target inside a symlinked directory.
    let real_dir = f.install.join("real");
    fs::create_dir_all(&real_dir).unwrap();
    fs::write(real_dir.join("b.AppImage"), b"b").unwrap();
    let dir_link = f.install.join("dirlink");
    symlink(&real_dir, &dir_link).unwrap();
    let inst = f.installer(Target::AppImage(dir_link.join("b.AppImage")));
    assert!(inst.stage(&ok).is_err());
}

#[test]
fn tampered_staged_file_is_not_applied() {
    let f = fixture();
    let live = f.install.join("a.AppImage");
    fs::write(&live, b"old").unwrap();
    let new = f.download("n.AppImage", b"new");
    let inst = f.installer(Target::AppImage(live.clone()));
    inst.stage(&new).unwrap();
    let staged = f.install.join(".a.AppImage.new");
    let mut h = fs::OpenOptions::new().append(true).open(&staged).unwrap();
    h.write_all(b"!").unwrap();
    drop(h);
    assert!(inst.apply_on_quit().is_err());
    assert_eq!(fs::read(&live).unwrap(), b"old");
    assert!(f.rec.calls.lock().unwrap().is_empty());
    assert_eq!(f.quits.load(Ordering::SeqCst), 0);
}

#[test]
fn windows_spawns_silent_setup_then_quits() {
    let f = fixture();
    let setup = f.download("MangoClient_2.0.1_x64-setup.exe", b"MZsetup");
    let inst = f.installer(Target::WindowsNsis);
    inst.stage(&setup).unwrap();
    inst.apply_on_quit().unwrap();
    let calls = f.rec.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.ends_with("MangoClient_2.0.1_x64-setup.exe"));
    assert_eq!(calls[0].1, vec![OsString::from("/S"), "/UPDATE".into()]);
    assert_eq!(f.quits.load(Ordering::SeqCst), 1);
}

#[test]
fn windows_rejects_non_exe_and_changed_file() {
    let f = fixture();
    let inst = f.installer(Target::WindowsNsis);
    assert!(inst.stage(&f.download("x.bin", b"data")).is_err());
    let setup = f.download("s.exe", b"MZ");
    inst.stage(&setup).unwrap();
    fs::write(&setup, b"MZ-swapped").unwrap();
    assert!(inst.apply_on_quit().is_err());
    assert!(f.rec.calls.lock().unwrap().is_empty());
}

fn app_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in entries {
        if name.ends_with('/') {
            w.add_directory(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
        } else {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(body).unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

fn good_zip() -> Vec<u8> {
    app_zip(&[
        ("MangoClient.app/Contents/", b""),
        ("MangoClient.app/Contents/MacOS/mangoclient", b"new bin"),
        ("MangoClient.app/Contents/Info.plist", b"plist"),
    ])
}

fn mac_bundle(f: &Fixture) -> PathBuf {
    let app = f.install.join("MangoClient.app");
    fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    fs::write(app.join("Contents/MacOS/mangoclient"), b"old bin").unwrap();
    app
}

#[test]
fn mac_swaps_bundle_with_backup_and_opens_it() {
    let f = fixture();
    let app = mac_bundle(&f);
    let inst = f.installer(Target::MacApp(app.clone()));
    inst.stage(&f.download("m.zip", &good_zip())).unwrap();
    assert_eq!(
        fs::read(app.join("Contents/MacOS/mangoclient")).unwrap(),
        b"old bin"
    );
    inst.apply_on_quit().unwrap();
    let bin = app.join("Contents/MacOS/mangoclient");
    assert_eq!(fs::read(&bin).unwrap(), b"new bin");
    #[cfg(unix)]
    assert_eq!(mode(&bin), 0o755);
    assert_eq!(
        fs::read(
            f.install
                .join(".MangoClient.app.old/Contents/MacOS/mangoclient")
        )
        .unwrap(),
        b"old bin"
    );
    assert!(!f.install.join(".MangoClient.app.update").exists());
    let calls = f.rec.calls.lock().unwrap();
    assert_eq!(calls[0].0, PathBuf::from("open"));
    assert_eq!(calls[0].1[0], OsString::from("-n"));
    assert_eq!(f.quits.load(Ordering::SeqCst), 1);
    drop(calls);
    inst.cleanup_backup();
    assert!(!f.install.join(".MangoClient.app.old").exists());
}

#[test]
fn mac_rejects_bad_archives_and_cleans_up() {
    let f = fixture();
    let app = mac_bundle(&f);
    let inst = f.installer(Target::MacApp(app.clone()));
    for bad in [
        app_zip(&[("evil.txt", b"x")]),
        app_zip(&[("../escape.app/Contents/MacOS/x", b"x")]),
        app_zip(&[
            ("A.app/Contents/MacOS/x", b"x"),
            ("B.app/Contents/MacOS/x", b"x"),
        ]),
        app_zip(&[("A.app/Contents/Info.plist", b"x")]),
    ] {
        assert!(inst.stage(&f.download("bad.zip", &bad)).is_err());
        assert!(!f.install.join(".MangoClient.app.update").exists());
    }
    assert!(!f.install.join("escape.app").exists());
    assert_eq!(
        fs::read(app.join("Contents/MacOS/mangoclient")).unwrap(),
        b"old bin"
    );
    assert!(inst.apply_on_quit().is_err());
}

#[test]
fn mac_rolls_back_when_second_rename_fails() {
    let f = fixture();
    let app = mac_bundle(&f);
    let inst = f.installer(Target::MacApp(app.clone()));
    inst.stage(&f.download("m.zip", &good_zip())).unwrap();
    // Break the staged tree so the swap's second rename fails.
    fs::remove_dir_all(f.install.join(".MangoClient.app.update/MangoClient.app")).unwrap();
    assert!(inst.apply_on_quit().is_err());
    assert_eq!(
        fs::read(app.join("Contents/MacOS/mangoclient")).unwrap(),
        b"old bin"
    );
    assert!(f.rec.calls.lock().unwrap().is_empty());
}

#[test]
fn apply_without_relaunch_swaps_but_neither_spawns_nor_quits() {
    let f = fixture();
    let live = f.install.join("MangoClient.AppImage");
    fs::write(&live, b"old build").unwrap();
    let new = f.download("new.AppImage", b"new build");
    let inst = f.installer(Target::AppImage(live.clone()));
    inst.stage(&new).unwrap();
    inst.apply_without_relaunch().unwrap();
    assert_eq!(fs::read(&live).unwrap(), b"new build");
    assert!(f.rec.calls.lock().unwrap().is_empty());
    assert_eq!(f.quits.load(Ordering::SeqCst), 0);
}

#[test]
fn windows_exit_install_runs_the_installer_without_the_update_flag() {
    let f = fixture();
    let setup = f.download("setup.exe", b"installer");
    let inst = f.installer(Target::WindowsNsis);
    inst.stage(&setup).unwrap();
    inst.apply_without_relaunch().unwrap();
    let calls = f.rec.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, vec![OsString::from("/S")]);
    assert_eq!(f.quits.load(Ordering::SeqCst), 0);
}

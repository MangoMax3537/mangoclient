#![allow(clippy::unwrap_used)]
//! Security review fixes: user-chosen paths (output hardening, one-shot
//! picker allow-list), log scrubbing before upload, tampered records.

use crate::tests_p4::Sink;
use crate::*;
use mango_core::Paths;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct Fx {
    pub tmp: tempfile::TempDir,
    pub svc: AppService,
}

impl Fx {
    pub fn root(&self) -> PathBuf {
        self.tmp.path().join("data")
    }
    pub fn scratch(&self) -> PathBuf {
        let d = self.tmp.path().join("scratch");
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    pub fn profile(&self) -> String {
        self.svc
            .profile_create("P".into(), "1.21.1".into(), "vanilla".into(), None)
            .unwrap()
            .id
    }
}

pub(crate) fn fx_with(strict: bool, ttl: Duration, dialog: CannedFileDialog) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        strict_paths: strict,
        path_grant_ttl: ttl,
        ..ServiceOptions::default()
    };
    let host = Host::system(false).with_dialog(Arc::new(dialog));
    let svc = AppService::with_host(
        Paths::new(tmp.path().join("data")),
        Arc::new(Sink::default()),
        opts,
        host,
    )
    .unwrap();
    Fx { tmp, svc }
}

fn png(dir: &Path, name: &str) -> PathBuf {
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([9, 9, 9, 255]));
    let p = dir.join(name);
    std::fs::write(&p, crate::icon_images::encode_png(&img).unwrap()).unwrap();
    p
}

async fn export(f: &Fx, id: &str, out: &Path) -> Result<FileExport, AppError> {
    f.svc
        .instance_export(
            id.into(),
            ExportFormat::Mrpack,
            false,
            out.display().to_string(),
        )
        .await
}

#[tokio::test]
async fn export_cannot_overwrite_files_in_the_data_root() {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let accounts = f.root().join("accounts.json");
    let before = std::fs::read(&accounts).unwrap();
    let e = export(&f, &id, &accounts).await.unwrap_err();
    assert_eq!(e.code, "export.invalid_path");
    assert_eq!(std::fs::read(&accounts).unwrap(), before);
    let e = f
        .svc
        .diagnostics_export(
            id.clone(),
            f.root().join("config.json").display().to_string(),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "export.invalid_path");
}

#[cfg(unix)]
#[tokio::test]
async fn export_refuses_symlinks_and_symlinked_folders_into_the_data_root() {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let accounts = f.root().join("accounts.json");
    let before = std::fs::read(&accounts).unwrap();
    let link = f.scratch().join("pack.mrpack");
    std::os::unix::fs::symlink(&accounts, &link).unwrap();
    assert_eq!(
        export(&f, &id, &link).await.unwrap_err().code,
        "export.invalid_path"
    );
    let dir_link = f.scratch().join("sneaky");
    std::os::unix::fs::symlink(f.root(), &dir_link).unwrap();
    let e = export(&f, &id, &dir_link.join("accounts.json"))
        .await
        .unwrap_err();
    assert_eq!(e.code, "export.invalid_path");
    assert_eq!(std::fs::read(&accounts).unwrap(), before);
}

#[tokio::test]
async fn export_refuses_unc_and_device_paths() {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    for p in [
        "\\\\server\\share\\a.mrpack",
        "\\\\?\\C:\\a.mrpack",
        "\\\\.\\pipe\\x",
    ] {
        let e = f
            .svc
            .instance_export(id.clone(), ExportFormat::Mrpack, false, p.into())
            .await
            .unwrap_err();
        assert_eq!(e.code, "path.unsupported", "{p}");
    }
}

#[tokio::test]
async fn export_refuses_the_bundled_resource_dir() {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let bundle = f.tmp.path().join("res");
    std::fs::create_dir_all(&bundle).unwrap();
    // Rebuild with that bundle dir.
    let opts = ServiceOptions {
        bundle_dir: bundle.clone(),
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        ..ServiceOptions::default()
    };
    let svc = AppService::new(Paths::new(f.root()), Arc::new(Sink::default()), opts).unwrap();
    let e = svc
        .instance_export(
            id,
            ExportFormat::Mrpack,
            false,
            bundle.join("x.mrpack").display().to_string(),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "export.invalid_path");
}

#[tokio::test]
async fn strict_export_needs_a_picked_save_path_once() {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("pack.mrpack");
    let f = fx_with(
        true,
        DEFAULT_GRANT_TTL,
        CannedFileDialog::default().with_save(Some(out.clone())),
    );
    let id = f.profile();
    // Typed path, never picked: refused, nothing written.
    let e = export(&f, &id, &out).await.unwrap_err();
    assert_eq!(e.code, "path.not_granted");
    assert!(!out.exists());

    let picked = f
        .svc
        .pick_save_file(None, "pack.mrpack".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(PathBuf::from(&picked), out);
    // A different path than the picked one is still refused.
    let other = out_dir.path().join("other.mrpack");
    assert_eq!(
        export(&f, &id, &other).await.unwrap_err().code,
        "path.not_granted"
    );
    export(&f, &id, &out).await.unwrap();
    assert!(out.exists());
    // Single use.
    assert_eq!(
        export(&f, &id, &out).await.unwrap_err().code,
        "path.not_granted"
    );
}

#[tokio::test]
async fn strict_icon_image_needs_the_picker_and_expires() {
    // Not picked: rejected, nothing read.
    let f = fx_with(true, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let pic = png(&f.scratch(), "any.png");
    let e = f
        .svc
        .profile_icon_set_image(id.clone(), pic.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");

    // Picked: works, up to the use count, and not for other files.
    let tmp = tempfile::tempdir().unwrap();
    let pic = png(tmp.path(), "a.png");
    let other = png(tmp.path(), "b.png");
    let f = fx_with(
        true,
        DEFAULT_GRANT_TTL,
        CannedFileDialog::new(Some(pic.clone()), None),
    );
    let id = f.profile();
    f.svc.pick_image_file(None).await.unwrap().unwrap();
    let e = f
        .svc
        .profile_icon_set_image(id.clone(), other.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");
    f.svc
        .profile_icon_set_image(id.clone(), pic.display().to_string())
        .await
        .unwrap();
    f.svc
        .profile_background_set_image(id.clone(), pic.display().to_string())
        .await
        .unwrap();
    let e = f
        .svc
        .profile_icon_set_image(id.clone(), pic.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted", "reuse beyond the use count");

    // Expiry.
    let f = fx_with(
        true,
        Duration::from_millis(1),
        CannedFileDialog::new(Some(pic.clone()), None),
    );
    let id = f.profile();
    f.svc.pick_image_file(None).await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let e = f
        .svc
        .profile_icon_set_image(id, pic.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted", "expired grant");
}

#[tokio::test]
async fn strict_import_needs_its_picker() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("p.mrpack");
    std::fs::write(&pack, b"not a zip").unwrap();
    let dl = tmp.path().join("dl");
    std::fs::create_dir_all(&dl).unwrap();

    let f = fx_with(
        true,
        DEFAULT_GRANT_TTL,
        CannedFileDialog::new(None, Some(dl.clone())).with_import(Some(pack.clone())),
    );
    let e = f
        .svc
        .instance_import_plan(pack.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");
    let e = f
        .svc
        .instance_import(pack.display().to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");

    f.svc.pick_import_file(None).await.unwrap().unwrap();
    // Granted: gets past the allow-list (and fails later as a bad archive).
    let e = f
        .svc
        .instance_import_plan(pack.display().to_string())
        .await
        .unwrap_err();
    assert_ne!(e.code, "path.not_granted");
}

#[tokio::test]
async fn non_strict_mode_keeps_typed_paths_working() {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let pic = png(&f.scratch(), "typed.png");
    f.svc
        .profile_icon_set_image(id.clone(), pic.display().to_string())
        .await
        .unwrap();
    let out = f.scratch().join("typed.mrpack");
    export(&f, &id, &out).await.unwrap();
}

/// The record was valid when it was stored; afterwards `mods/` was swapped
/// for a symlink to a folder outside the instance holding a same-named file.
#[cfg(unix)]
async fn swapped_mods_dir() -> (Fx, String, PathBuf, PathBuf) {
    let f = fx_with(false, DEFAULT_GRANT_TTL, CannedFileDialog::default());
    let id = f.profile();
    let inst = f.root().join("instances").join(&id);
    std::fs::create_dir_all(inst.join("mods")).unwrap();
    std::fs::write(inst.join("mods/foo.jar"), b"mine").unwrap();
    f.svc.content_list(id.clone()).await.unwrap();

    let outside = f.tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let victim = outside.join("foo.jar");
    std::fs::write(&victim, b"VICTIM").unwrap();
    std::fs::remove_dir_all(inst.join("mods")).unwrap();
    std::os::unix::fs::symlink(&outside, inst.join("mods")).unwrap();
    (f, id, victim, outside)
}

#[cfg(unix)]
#[tokio::test]
async fn uninstall_never_deletes_through_a_symlinked_content_folder() {
    let (f, id, victim, _) = swapped_mods_dir().await;
    f.svc
        .content_uninstall(id, "local:foo.jar".into())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&victim).unwrap(), b"VICTIM");
}

#[cfg(unix)]
#[tokio::test]
async fn toggle_never_renames_through_a_symlinked_content_folder() {
    let (f, id, victim, outside) = swapped_mods_dir().await;
    // The record no longer has a usable path, so the toggle is refused.
    assert!(f
        .svc
        .content_toggle(id, "local:foo.jar".into(), false)
        .await
        .is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"VICTIM");
    assert!(!outside.join("foo.jar.disabled").exists());
}

#[test]
fn rebased_records_cannot_leave_the_instance() {
    let new_dir = PathBuf::from("/data/instances/new");
    let rec = |f: &str| mango_core::ModRecord {
        file: Some(f.to_string()),
        ..Default::default()
    };
    let out = crate::backups::rebase_records_for_test(
        vec![
            rec("/old/inst/mods/a.jar"),
            rec("/old/inst/../../../etc/passwd"),
            rec("/old/inst/mods/../../x"),
        ],
        "/old/inst",
        &new_dir,
    );
    assert_eq!(
        out[0].file.as_deref(),
        Some("/data/instances/new/mods/a.jar")
    );
    assert_eq!(out[1].file, None);
    assert_eq!(out[2].file, None);
}

#![allow(clippy::unwrap_used)]
//! Part 8 of `docs/ipc.md`: file dialogs, bundle dir resolution, background
//! dim and blur.

use crate::tests_p4::Sink;
use crate::*;
use mango_core::Paths;
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

fn service(dialog: Option<Arc<dyn FileDialog>>) -> (tempfile::TempDir, AppService) {
    let tmp = tempfile::tempdir().unwrap();
    let o = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        manifest_url: "http://127.0.0.1:1/manifest.json".into(),
        ..ServiceOptions::default()
    };
    let mut host = Host::system(false);
    if let Some(d) = dialog {
        host = host.with_dialog(d);
    }
    let svc = AppService::with_host(
        Paths::new(tmp.path().join("data")),
        Arc::new(Sink::default()),
        o,
        host,
    )
    .unwrap();
    (tmp, svc)
}

#[tokio::test]
async fn dialogs_return_paths_or_null() {
    let dlg = Arc::new(CannedFileDialog::new(
        Some(PathBuf::from("/x/pic.png")),
        None,
    ));
    let (_t, svc) = service(Some(dlg.clone()));
    assert_eq!(
        svc.pick_image_file(Some("Pick".into())).await.unwrap(),
        Some("/x/pic.png".to_string())
    );
    assert_eq!(svc.pick_folder(None).await.unwrap(), None);
    assert_eq!(
        dlg.calls(),
        vec![("image", Some("Pick".to_string())), ("folder", None)]
    );
    // default host: always cancelled
    let (_t2, plain) = service(None);
    assert_eq!(plain.pick_image_file(None).await.unwrap(), None);
}

#[test]
fn bundle_dir_resolution_order() {
    let res = tempfile::tempdir().unwrap();
    let env_dir = tempfile::tempdir().unwrap();
    // no packaged dir yet: dev fallback
    let dev = resolve_bundle_dir(None, Some(res.path()));
    assert!(dev.ends_with("bundle/mangoconfig"));
    // packaged dir wins over dev
    std::fs::create_dir(res.path().join("mangoconfig")).unwrap();
    assert_eq!(
        resolve_bundle_dir(None, Some(res.path())),
        res.path().join("mangoconfig")
    );
    // empty env is ignored
    assert_eq!(
        resolve_bundle_dir(Some(OsString::new()), Some(res.path())),
        res.path().join("mangoconfig")
    );
    // env wins over everything
    assert_eq!(
        resolve_bundle_dir(Some(env_dir.path().into()), Some(res.path())),
        env_dir.path()
    );
    // no resource dir at all
    assert_eq!(resolve_bundle_dir(None, None), dev);
}

#[test]
fn background_style_validates_persists_and_resets() {
    let (_t, svc) = service(None);
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    // default: no override fields
    assert_eq!(
        serde_json::to_value(&p.background).unwrap(),
        json!({"kind": "auto"})
    );
    let v = svc
        .profile_background_set_style(p.id.clone(), Some(0.4), Some(12.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "auto", "dim": 0.4, "blur": 12.0})
    );
    // partial update keeps the other value
    let v = svc
        .profile_background_set_style(p.id.clone(), None, Some(0.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "auto", "dim": 0.4, "blur": 0.0})
    );
    // boundaries are valid; out of range is rejected, nothing changes
    svc.profile_background_set_style(p.id.clone(), Some(1.0), Some(24.0))
        .unwrap();
    for (d, b) in [
        (Some(1.01), None),
        (Some(-0.01), None),
        (None, Some(24.5)),
        (None, Some(-1.0)),
        (Some(f64::NAN), None),
        (None, Some(f64::INFINITY)),
    ] {
        let e = svc
            .profile_background_set_style(p.id.clone(), d, b)
            .unwrap_err();
        assert_eq!(e.code, "background.bad_style");
    }
    let after = svc
        .profile_background_set_style(p.id.clone(), None, None)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&after.background).unwrap(),
        json!({"kind": "auto", "dim": 1.0, "blur": 24.0})
    );
    // unknown profile
    assert_eq!(
        svc.profile_background_set_style("nope".into(), Some(0.1), None)
            .unwrap_err()
            .code,
        svc.profile_background_set_style("nope".into(), None, None)
            .unwrap_err()
            .code
    );
    // clear resets the override
    let v = svc.profile_background_clear(p.id.clone(), false).unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "auto"})
    );
}

#[test]
fn legacy_profile_without_style_loads() {
    let raw =
        json!({"id": "123e4567-e89b-42d3-a456-426614174000", "name": "Old", "background": "image"});
    let p = mango_core::Profile::from_stored(&raw, 1).unwrap();
    assert_eq!(p.background_dim, None);
    let bad = json!({"id": "123e4567-e89b-42d3-a456-426614174000", "backgroundDim": 7});
    assert_eq!(
        mango_core::Profile::from_stored(&bad, 1)
            .unwrap()
            .background_dim,
        None
    );
}

#[test]
fn background_kinds_wallpaper_and_panorama() {
    let (_t, svc) = service(None);
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    let kind = |k: &str, w: Option<&str>| {
        svc.profile_background_set_kind(p.id.clone(), k.into(), w.map(String::from))
    };
    assert_eq!(
        serde_json::to_value(&kind("panorama", None).unwrap().background).unwrap(),
        json!({"kind": "panorama"})
    );
    svc.profile_background_set_style(p.id.clone(), Some(0.3), None)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&kind("wallpaper", Some("end-city-2")).unwrap().background).unwrap(),
        json!({"kind": "wallpaper", "id": "end-city-2", "dim": 0.3})
    );
    assert_eq!(
        serde_json::to_value(&kind("none", None).unwrap().background).unwrap(),
        json!({"kind": "none"})
    );
    assert_eq!(
        serde_json::to_value(&kind("auto", None).unwrap().background).unwrap(),
        json!({"kind": "auto", "dim": 0.3})
    );
    for w in [None, Some(""), Some("Up"), Some("a/b"), Some("..")] {
        assert_eq!(
            kind("wallpaper", w).unwrap_err().code,
            "background.bad_wallpaper"
        );
    }
    assert_eq!(
        kind("wallpaper", Some(&"a".repeat(65))).unwrap_err().code,
        "background.bad_wallpaper"
    );
    assert_eq!(kind("image", None).unwrap_err().code, "background.bad_kind");
    assert_eq!(
        kind("none", Some("x")).unwrap_err().code,
        "background.bad_kind"
    );
    // survives a reload and legacy values still load
    let stored = json!({"id": p.id, "background": "wallpaper:end-city-2"});
    let again = mango_core::Profile::from_stored(&stored, 1).unwrap();
    assert_eq!(again.background.as_deref(), Some("wallpaper:end-city-2"));
    let bad = json!({"id": p.id, "background": "wallpaper:../x"});
    assert_eq!(
        mango_core::Profile::from_stored(&bad, 1)
            .unwrap()
            .background,
        None
    );
}

#[tokio::test]
async fn icon_source_and_modpack_logo_restore() {
    let (t, svc) = service(None);
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    assert_eq!(p.icon_source, None);
    assert_eq!(
        svc.profile_icon_use_modpack_logo(p.id.clone())
            .await
            .unwrap_err()
            .code,
        "icon.no_modpack_logo"
    );
    // simulate modpack_install: the logo as cover and pack copy
    let img = image::RgbaImage::from_fn(4, 4, |_, _| image::Rgba([9, 9, 9, 255]));
    let png = crate::icon_images::encode_png(&img).unwrap();
    let paths = Paths::new(t.path().join("data"));
    std::fs::create_dir_all(paths.covers()).unwrap();
    std::fs::write(paths.pack_logo_file(&p.id).unwrap(), &png).unwrap();
    // custom picture
    let up = t.path().join("up.png");
    std::fs::write(&up, &png).unwrap();
    let v = svc
        .profile_icon_set_image(p.id.clone(), up.display().to_string())
        .await
        .unwrap();
    assert_eq!(v.icon_source.as_deref(), Some("custom"));
    let v = svc
        .profile_icon_use_modpack_logo(p.id.clone())
        .await
        .unwrap();
    assert_eq!(v.icon_source.as_deref(), Some("modpack"));
    assert!(matches!(v.icon, Some(InstanceIcon::Image)));
    let v = svc.profile_icon_clear(p.id.clone()).unwrap();
    assert_eq!(v.icon_source, None);
    // the pack copy survives clearing the icon
    assert!(paths.pack_logo_file(&p.id).unwrap().is_file());
}

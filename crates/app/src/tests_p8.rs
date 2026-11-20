#![allow(clippy::unwrap_used)]
//! Updater wiring (installer injection), launch isolation from the real
//! network, and strict profile/loader validation.

use crate::tests_p2::{json_ok, serve, Handler, Req};
use crate::tests_p4::Sink;
use crate::*;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use mango_core::Paths;
use serde_json::json;
use sha1::{Digest as _, Sha1};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

async fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not reached in time");
}

// ---------- updater ----------

#[derive(Default)]
struct FakeInstaller {
    staged: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    applied: Mutex<usize>,
}

impl mango_updater::Installer for FakeInstaller {
    fn stage(&self, file: &Path) -> mango_updater::Result<()> {
        let bytes = std::fs::read(file).unwrap();
        self.staged
            .lock()
            .unwrap()
            .push((file.to_path_buf(), bytes));
        Ok(())
    }
    fn apply_on_quit(&self) -> mango_updater::Result<()> {
        *self.applied.lock().unwrap() += 1;
        Ok(())
    }
}

impl FakeInstaller {
    fn staged(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.staged.lock().unwrap().clone()
    }
    fn applied(&self) -> usize {
        *self.applied.lock().unwrap()
    }
}

fn payload() -> Vec<u8> {
    (0..4096u32).map(|i| (i % 251) as u8).collect()
}

struct UpdFx {
    _tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    installer: Arc<FakeInstaller>,
}

/// A service that talks to a mock feed signed with `signer`, trusting
/// `trusted`. `auto` runs the launch-time check (`start_background`).
async fn upd_fx(signer: &SigningKey, trusted: &SigningKey, auto: bool) -> UpdFx {
    let data = payload();
    let sha = hex::encode(sha2::Sha256::digest(&data));
    let msg = mango_updater::update_message("9.0.0", "linux-x86_64-appimage", &sha);
    let sig = B64.encode(signer.sign(&msg).to_bytes());
    let (base, _seen) = serve(move |base| {
        let manifest = json!({
            "version": "9.0.0",
            "platforms": {"linux-x86_64-appimage": {
                "url": format!("{base}/update.AppImage"),
                "sha256": sha, "size": data.len(), "signature": sig}}
        });
        let body = data.clone();
        Arc::new(move |r: &Req| match r.path.as_str() {
            "/latest.json" => json_ok(manifest.clone()),
            "/update.AppImage" => (200, body.clone()),
            _ => (404, Vec::new()),
        })
    })
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let installer = Arc::new(FakeInstaller::default());
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: false,
        auto_update_check: auto,
        update_manifest_url: format!("{base}/latest.json"),
        update_public_key: B64.encode(trusted.verifying_key().to_bytes()),
        update_platform: Some(mango_updater::Platform {
            os: "linux".into(),
            arch: "x86_64".into(),
            appimage: true,
        }),
        update_policy: Some(mango_updater::UrlPolicy::insecure_for_tests(&["127.0.0.1"])),
        update_relaunch_delay: Duration::from_millis(10),
        app_version: "1.0.0".into(),
        partners: Vec::new(),
        ..ServiceOptions::default()
    };
    let host = Host::system(false).with_installer(installer.clone());
    let svc = AppService::with_host(Paths::new(tmp.path()), sink.clone(), opts, host).unwrap();
    svc.config_set(json!({"telemetry": false})).unwrap();
    UpdFx {
        _tmp: tmp,
        svc,
        sink,
        installer,
    }
}

#[tokio::test]
async fn verified_update_is_staged_once_and_applied_on_install() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let f = upd_fx(&key, &key, false).await;
    let st = f.svc.update_check().await.unwrap();
    assert_eq!((st.state, st.version.as_deref()), ("ready", Some("9.0.0")));
    let staged = f.installer.staged();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].1, payload());
    assert!(staged[0].0.starts_with(updates_dir(f.svc.paths())));
    assert_eq!(f.installer.applied(), 0, "apply waits for the user");
    // Another check while ready neither re-stages nor applies.
    f.svc.update_check().await.unwrap();
    assert_eq!(f.installer.staged().len(), 1);

    assert!(f.svc.update_install().unwrap());
    assert_eq!((f.installer.staged().len(), f.installer.applied()), (1, 1));
}

#[tokio::test]
async fn unverified_update_never_reaches_the_installer() {
    let signer = SigningKey::from_bytes(&[7; 32]);
    let trusted = SigningKey::from_bytes(&[8; 32]);
    let f = upd_fx(&signer, &trusted, false).await;
    let st = f.svc.update_check().await.unwrap();
    assert_eq!(st.state, "error");
    assert!(f.installer.staged().is_empty());
    assert!(!f.svc.update_install().unwrap());
    assert_eq!(f.installer.applied(), 0);
    let leftovers = std::fs::read_dir(updates_dir(f.svc.paths()))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "the rejected download is deleted");
}

#[tokio::test]
async fn launch_time_check_applies_through_the_installer() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let f = upd_fx(&key, &key, true).await;
    f.svc.start_background();
    let inst = f.installer.clone();
    wait_for(move || inst.applied() == 1).await;
    assert_eq!(f.installer.staged().len(), 1);
    // Exactly once, also after the periodic machinery had time to run.
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(f.installer.applied(), 1);
    assert!(f
        .sink
        .events()
        .iter()
        .any(|e| matches!(e, Event::UpdateStatus(s) if s.state == "ready")));
}

// ---------- strict validation ----------

fn plain() -> (tempfile::TempDir, AppService) {
    let tmp = tempfile::tempdir().unwrap();
    let opts = ServiceOptions {
        open_folders: false,
        auto_update_check: false,
        partners: Vec::new(),
        ..ServiceOptions::default()
    };
    let svc = AppService::new(Paths::new(tmp.path()), Arc::new(Sink::default()), opts).unwrap();
    (tmp, svc)
}

#[tokio::test]
async fn profile_update_rejects_immutable_unknown_and_owned_keys() {
    let (_t, svc) = plain();
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "fabric".into(), None)
        .unwrap();
    for bad in [
        "id",
        "created",
        "icon",
        "background",
        "mods",
        "session",
        "nope",
    ] {
        let e = svc
            .profile_update(p.id.clone(), json!({"name": "X", bad: 1}))
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation", "{bad}");
    }
    // Nothing was applied by a rejected patch.
    let st = svc.app_state().unwrap();
    assert_eq!(st.profiles.iter().find(|x| x.id == p.id).unwrap().name, "A");
    // Derived view fields stay tolerated so a UI may spread a view.
    let u = svc
        .profile_update(
            p.id.clone(),
            json!({"name": "B", "modCount": 3, "coverUrl": null}),
        )
        .await
        .unwrap();
    assert_eq!(u.name, "B");
    let e = svc
        .profile_update(p.id.clone(), json!({"loader": "bogus"}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "validation");
    assert!(e.message.contains("bogus"));
}

#[tokio::test]
async fn loader_must_be_a_known_loader() {
    let (_t, svc) = plain();
    for bad in ["", "Fabric", "liteloader", "../x"] {
        let e = svc
            .profile_create("A".into(), "1.21.1".into(), bad.into(), None)
            .unwrap_err();
        assert_eq!(e.code, "validation", "create {bad:?}");
        let e = svc
            .loader_versions(bad.into(), "1.21.1".into())
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation", "versions {bad:?}");
    }
    assert!(svc
        .loader_versions("vanilla".into(), "1.21.1".into())
        .await
        .unwrap()
        .is_empty());
}

// ---------- launch isolated from the real network ----------

#[cfg(unix)]
#[tokio::test]
async fn launch_runs_end_to_end_against_mock_endpoints() {
    use std::os::unix::fs::PermissionsExt;

    let client = b"fake client jar".to_vec();
    let lib = b"fake library jar".to_vec();
    let asset = b"asset bytes".to_vec();
    let sha1 = |b: &[u8]| hex::encode(Sha1::digest(b));
    let ahash = sha1(&asset);
    let index = serde_json::to_vec(
        &json!({"objects": {"minecraft/a.txt": {"hash": ahash, "size": asset.len()}}}),
    )
    .unwrap();

    let routes_asset = asset.clone();
    let make = {
        let (client, lib, index) = (client.clone(), lib.clone(), index.clone());
        move |base: &str| -> Handler {
            let version = json!({
                "id": "1.21", "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {"majorVersion": 17},
                "downloads": {"client": {"url": format!("{base}/client.jar"),
                    "sha1": sha1(&client), "size": client.len()}},
                "assets": "17",
                "assetIndex": {"id": "17", "url": format!("{base}/idx.json"),
                    "sha1": sha1(&index), "size": index.len()},
                "libraries": [{"name": "a:b:1", "downloads": {"artifact": {
                    "path": "a/b/1/b-1.jar", "url": format!("{base}/lib/a.jar"),
                    "sha1": sha1(&lib), "size": lib.len()}}}]
            });
            let manifest = json!({
                "latest": {"release": "1.21", "snapshot": "1.21"},
                "versions": [{"id": "1.21", "type": "release",
                    "url": format!("{base}/v/1.21.json"), "releaseTime": "2024-06-13T00:00:00Z"}]
            });
            let asset_path = format!("/res/{}/{}", &ahash[..2], ahash);
            Arc::new(move |r: &Req| match r.path.as_str() {
                "/manifest.json" => json_ok(manifest.clone()),
                "/v/1.21.json" => json_ok(version.clone()),
                "/client.jar" => (200, client.clone()),
                "/lib/a.jar" => (200, lib.clone()),
                "/idx.json" => (200, index.clone()),
                p if p == asset_path => (200, routes_asset.clone()),
                _ => (404, Vec::new()),
            })
        }
    };
    let (base, seen) = serve(make).await;

    let tmp = tempfile::tempdir().unwrap();
    let java = tmp.path().join("fakejava.sh");
    std::fs::write(
        &java,
        format!("#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then\n  echo 'openjdk version \"21.0.1\" 2023-10-17' >&2\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > '{}'\necho 'Setting user: Player'\nsleep 1\nexit 0\n", tmp.path().join("args.txt").display()),
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Anything not served by the mock must fail loudly, never reach a real host.
    let dead = "http://127.0.0.1:1";
    let sink = Arc::new(Sink::default());
    let opts = ServiceOptions {
        open_folders: false,
        auto_update_check: false,
        partners: Vec::new(),
        bundle_dir: tmp.path().join("no-bundle"),
        manifest_url: format!("{base}/manifest.json"),
        resources_url: format!("{base}/res"),
        modrinth_api: format!("{base}/modrinth"),
        adoptium_base: format!("{dead}/adoptium"),
        loader_endpoints: mango_loaders::Endpoints {
            fabric_meta: format!("{dead}/fabric"),
            quilt_meta: format!("{dead}/quilt"),
            neoforge_maven: format!("{dead}/neoforge"),
            forge_maven: format!("{dead}/forge"),
        },
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        ..ServiceOptions::default()
    };
    let svc = AppService::with_host(
        Paths::new(tmp.path().join("data")),
        sink.clone(),
        opts,
        Host::system(false),
    )
    .unwrap();
    svc.config_set(
        json!({"javaPath": java.display().to_string(), "telemetry": false, "ram": 3072}),
    )
    .unwrap();
    let acc = svc.account_add_offline("Player_1".into()).unwrap();
    svc.account_select(acc.uuid).unwrap();
    let p = svc
        .profile_create("V".into(), "1.21".into(), "vanilla".into(), None)
        .unwrap();

    let res = svc.game_launch(p.id.clone(), None).await.unwrap();
    assert!(res.pid > 0);
    assert_eq!(res.version_id, "1.21");

    let states = || {
        let mut out: Vec<LaunchState> = Vec::new();
        for e in sink.events() {
            if let Event::LaunchState(s) = e {
                if s.profile_id == p.id && out.last() != Some(&s.state) {
                    out.push(s.state);
                }
            }
        }
        out
    };
    wait_for(|| states().last() == Some(&LaunchState::Stopped)).await;
    assert_eq!(
        states(),
        vec![
            LaunchState::Preparing,
            LaunchState::Java,
            LaunchState::Loader,
            LaunchState::Downloading,
            LaunchState::Account,
            LaunchState::Launching,
            LaunchState::Running,
            LaunchState::Stopped,
        ]
    );
    // The launch heap is the profile's effective RAM: first the config
    // default, then an explicit override.
    let xmx = || {
        std::fs::read_to_string(tmp.path().join("args.txt"))
            .unwrap()
            .lines()
            .find(|l| l.starts_with("-Xmx"))
            .map(str::to_string)
    };
    assert_eq!(p.effective_ram_mb, 3072);
    assert_eq!(xmx().as_deref(), Some("-Xmx3072M"));
    let v = svc
        .profile_update(p.id.clone(), json!({"ram": 2048}))
        .await
        .unwrap();
    assert_eq!(v.effective_ram_mb, 2048);
    svc.game_launch(p.id.clone(), None).await.unwrap();
    wait_for(|| xmx().as_deref() == Some("-Xmx2048M")).await;
    wait_for(|| !svc.game_running().contains(&p.id)).await;

    // Everything came from the mock; nothing unexpected was asked for.
    let paths: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.path.clone())
        .collect();
    for want in [
        "/manifest.json",
        "/v/1.21.json",
        "/client.jar",
        "/lib/a.jar",
        "/idx.json",
    ] {
        assert!(paths.iter().any(|p| p == want), "missing {want}: {paths:?}");
    }
    assert!(paths.iter().any(|p| p.starts_with("/res/")), "{paths:?}");
    for p in &paths {
        assert!(
            !p.starts_with("/adoptium") && !p.starts_with("/fabric"),
            "{p}"
        );
    }
    assert!(!svc.is_busy(&p.id));
}

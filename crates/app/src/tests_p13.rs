#![allow(clippy::unwrap_used)]
//! Part 13 of `docs/ipc.md`: skins, capes and the skin library, per-instance
//! Java and window settings, the Java list/installer and the panorama pool.
//! Mock servers only; no real Microsoft, Mojang or Adoptium endpoint.

use crate::image_protocol::{resolve, Image, ImageError};
use crate::skins::{validate_skin_png, MAX_LIBRARY_SKINS};
use crate::tests_p2::{json_ok, serve, Handler, Req};
use crate::*;
use base64::Engine;
use mango_auth::MemoryDeviceKey;
use mango_core::Paths;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const UUID: &str = "0123456789abcdef0123456789abcdef";
const OTHER: &str = "fedcba9876543210fedcba9876543210";

#[derive(Default)]
struct Sink(Mutex<Vec<Event>>);

impl EventSink for Sink {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl Sink {
    fn skin_updates(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, Event::SkinUpdated(s) if s.uuid == UUID))
            .count()
    }
    fn names(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|e| format!("{} {}", e.name(), e.payload()))
            .collect()
    }
}

async fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..400 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not reached in time");
}

fn skin_png(w: u32, h: u32, tag: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([tag, (x % 256) as u8, y as u8, 255])
    });
    crate::icon_images::encode_png(&img).unwrap()
}

/// A cape PNG (only the signature matters to the cache).
fn cape_png(tag: u8) -> Vec<u8> {
    skin_png(64, 32, tag)
}

struct Mock {
    skin_ver: u32,
    active_cape: Option<String>,
    other_slim: bool,
    fail_profile: bool,
    fail_session: bool,
    upload_status: u16,
    lookup_status: u16,
    lookup_legacy_ok: bool,
    uploads: Vec<(String, Vec<u8>)>,
    cape_calls: Vec<(String, String)>,
}

impl Default for Mock {
    fn default() -> Self {
        Mock {
            skin_ver: 0,
            active_cape: Some("cape-1".into()),
            other_slim: false,
            fail_profile: false,
            fail_session: false,
            upload_status: 200,
            lookup_status: 200,
            lookup_legacy_ok: false,
            uploads: Vec::new(),
            cape_calls: Vec::new(),
        }
    }
}

fn session_json(base: &str, skin_path: &str, slim: bool, id: &str) -> Value {
    let textures = json!({"textures": {"SKIN": {
        "url": format!("{base}{skin_path}"),
        "metadata": if slim { json!({"model": "slim"}) } else { json!({}) }
    }}});
    json!({"id": id, "name": "x", "properties": [{
        "name": "textures",
        "value": base64::engine::general_purpose::STANDARD.encode(textures.to_string())
    }]})
}

fn mock_handler(mock: Arc<Mutex<Mock>>) -> impl FnOnce(&str) -> Handler {
    move |base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let mut m = mock.lock().unwrap();
            let path = r.path.split('?').next().unwrap_or("").to_string();
            match (r.method.as_str(), path.as_str()) {
                (_, "/live/token") => {
                    json_ok(json!({"access_token": "live-at", "refresh_token": "rt-1"}))
                }
                (_, "/xbl") => json_ok(json!({"Token": "xbl-token"})),
                (_, "/xsts") => {
                    json_ok(json!({"Token": "xsts", "DisplayClaims": {"xui": [{"uhs": "uhs"}]}}))
                }
                (_, "/mc/login") => {
                    json_ok(json!({"access_token": "mc-token", "expires_in": 3600}))
                }
                (_, "/mc/ent") => json_ok(json!({"items": [{"name": "game_minecraft"}]})),
                ("GET", "/mc/profile") => {
                    if m.fail_profile {
                        return (500, vec![]);
                    }
                    let cape = |id: &str, alias: &str, n: u8| {
                        json!({"id": id, "alias": alias, "url": format!("{base}/tex/cape{n}"),
                               "state": if m.active_cape.as_deref() == Some(id) { "ACTIVE" } else { "INACTIVE" }})
                    };
                    json_ok(json!({
                        "id": UUID, "name": "Steve",
                        "skins": [{"state": "ACTIVE", "variant": "CLASSIC",
                                   "url": format!("{base}/tex/skin-{}", m.skin_ver)}],
                        "capes": [cape("cape-1", "Migrator", 1), cape("cape-2", "Vanilla", 2)],
                    }))
                }
                ("POST", "/mc/skins") => {
                    m.uploads.push((r.head.clone(), r.body.clone()));
                    if m.upload_status != 200 {
                        return (m.upload_status, b"{}".to_vec());
                    }
                    m.skin_ver += 1;
                    json_ok(json!({}))
                }
                ("PUT", "/mc/capes") => {
                    let body = String::from_utf8_lossy(&r.body).to_string();
                    let v: Value = serde_json::from_str(&body).unwrap_or_default();
                    m.active_cape = v["capeId"].as_str().map(str::to_string);
                    m.cape_calls.push(("PUT".into(), body));
                    json_ok(json!({}))
                }
                ("DELETE", "/mc/capes") => {
                    m.active_cape = None;
                    m.cape_calls.push(("DELETE".into(), String::new()));
                    json_ok(json!({}))
                }
                (_, p) if p.starts_with("/tex/skin-") => {
                    (200, skin_png(64, 64, m.skin_ver as u8 + 1))
                }
                (_, "/tex/other") => (200, skin_png(64, 64, 99)),
                (_, "/tex/cape1") => (200, cape_png(1)),
                (_, "/tex/cape2") => (200, cape_png(2)),
                (_, p) if p.starts_with("/session/") => {
                    if m.fail_session {
                        return (500, vec![]);
                    }
                    let id = p.trim_start_matches("/session/");
                    if id == OTHER {
                        json_ok(session_json(&base, "/tex/other", m.other_slim, OTHER))
                    } else {
                        json_ok(session_json(
                            &base,
                            &format!("/tex/skin-{}", m.skin_ver),
                            false,
                            UUID,
                        ))
                    }
                }
                ("GET", "/lookup/Notch") if m.lookup_status == 200 => {
                    json_ok(json!({"id": OTHER, "name": "Notch"}))
                }
                ("GET", p) if p.starts_with("/lookup/") => (m.lookup_status.max(404), vec![]),
                ("GET", "/lookup-old/Notch") if m.lookup_legacy_ok => {
                    json_ok(json!({"id": OTHER, "name": "Notch"}))
                }
                ("GET", p) if p.starts_with("/lookup-old/") => (204, vec![]),
                _ => (404, vec![]),
            }
        })
    }
}

struct Fx {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    mock: Arc<Mutex<Mock>>,
    seen: Arc<Mutex<Vec<Req>>>,
    keys: Arc<MemoryDeviceKey>,
    ep: mango_auth::Endpoints,
    dialog: Arc<CannedFileDialog>,
}

fn endpoints(base: &str) -> mango_auth::Endpoints {
    mango_auth::Endpoints {
        live_token: format!("{base}/live/token"),
        xbl_auth: format!("{base}/xbl"),
        xsts_auth: format!("{base}/xsts"),
        mc_login: format!("{base}/mc/login"),
        mc_profile: format!("{base}/mc/profile"),
        mc_entitlements: format!("{base}/mc/ent"),
        mc_skins: format!("{base}/mc/skins"),
        mc_capes_active: format!("{base}/mc/capes"),
        profile_lookup: format!("{base}/lookup"),
        profile_lookup_legacy: format!("{base}/lookup-old"),
        session_profile: format!("{base}/session"),
        retries: 0,
        backoff: Duration::from_millis(1),
        ..mango_auth::Endpoints::default()
    }
}

fn open_svc(
    dir: &Path,
    ep: &mango_auth::Endpoints,
    keys: Arc<MemoryDeviceKey>,
    strict: bool,
    image: Option<PathBuf>,
) -> (AppService, Arc<Sink>, Arc<CannedFileDialog>) {
    let sink = Arc::new(Sink::default());
    let redirect = ep.redirect_uri.clone();
    let login = Arc::new(CannedLoginWindow::new(LoginOutcome::Code(format!(
        "{redirect}?code=c&state={{state}}"
    ))));
    let dialog = Arc::new(CannedFileDialog::new(image, None));
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        auth_endpoints: ep.clone(),
        strict_paths: strict,
        skin_prefetch_delay: Duration::from_millis(10),
        manifest_url: format!("{}/manifest", ep.session_profile),
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        ..ServiceOptions::default()
    };
    let host = Host::system(false)
        .with_login(login)
        .with_device_keys(keys)
        .with_dialog(dialog.clone());
    let svc = AppService::with_host(Paths::new(dir), sink.clone(), opts, host).unwrap();
    (svc, sink, dialog)
}

/// A service with a Microsoft account signed in and its skin cached.
async fn fx(strict: bool, image: Option<PathBuf>) -> Fx {
    let mock = Arc::new(Mutex::new(Mock::default()));
    let (base, seen) = serve(mock_handler(mock.clone())).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    let keys = Arc::new(MemoryDeviceKey::new());
    let (svc, sink, dialog) = open_svc(tmp.path(), &ep, keys.clone(), strict, image);
    let a = svc.account_sign_in_microsoft().await.unwrap().unwrap();
    assert_eq!(a.uuid, UUID);
    let s = sink.clone();
    wait_for(move || s.skin_updates() >= 1).await;
    Fx {
        tmp,
        svc,
        sink,
        mock,
        seen,
        keys,
        ep,
        dialog,
    }
}

impl Fx {
    fn paths(&self) -> &Paths {
        self.svc.paths()
    }
    fn head_url(&self) -> String {
        self.svc.app_state().unwrap().accounts[0].head_url.clone()
    }
    fn write_png(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.tmp.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p.display().to_string()
    }
    fn calls(&self, method: &str, path: &str) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method == method && r.path.split('?').next() == Some(path))
            .count()
    }
}

fn image_bytes(paths: &Paths, url: &str) -> Vec<u8> {
    match resolve(paths, url).unwrap() {
        Image::Bytes { data, .. } => data,
        Image::File { path, .. } => std::fs::read(path).unwrap(),
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------- skins: fetch and cache

#[tokio::test]
async fn sign_in_fetches_and_caches_the_skin_and_versions_the_head() {
    let f = fx(false, None).await;
    let cache = f.paths().cache().join("skins").join(format!("{UUID}.json"));
    assert!(cache.is_file(), "skin cached after sign-in");
    let head = f.head_url();
    assert!(
        head.starts_with(&format!("mangoimg://head/{UUID}?v=")),
        "{head}"
    );
    // The versioned head resolves and renders the real skin, not Steve.
    let rendered = image_bytes(f.paths(), &head);
    assert!(rendered.starts_with(&[0x89, b'P', b'N', b'G']));

    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    assert!(info.can_edit);
    assert_eq!(info.source, SkinSourceName::Account);
    assert!(!info.slim);
    assert!(info
        .skin_url
        .starts_with(&format!("mangoimg://skin/{UUID}?v=")));
    assert_eq!(info.capes.len(), 2);
    assert_eq!(info.capes[0].id, "cape-1");
    assert_eq!(info.capes[0].alias, "Migrator");
    assert!(info.capes[0].active && !info.capes[1].active);
    assert_eq!(info.cape_url.as_deref(), Some(info.capes[0].url.as_str()));
    // Skin and cape routes serve the cached textures.
    assert_eq!(image_bytes(f.paths(), &info.skin_url), skin_png(64, 64, 1));
    assert_eq!(image_bytes(f.paths(), &info.capes[1].url), cape_png(2));

    // JSON shape of the wire type.
    let v = serde_json::to_value(&info).unwrap();
    assert!(v["skinUrl"].is_string() && v["capeUrl"].is_string());
    assert_eq!(v["canEdit"], true);
    assert_eq!(v["source"], "account");
    assert_eq!(v["capes"][0]["alias"], "Migrator");
}

#[tokio::test]
async fn repeated_fetches_only_announce_real_changes_and_a_failure_keeps_the_cache() {
    let f = fx(false, None).await;
    let before = f.sink.skin_updates();
    f.svc.skin_get(UUID.into()).await.unwrap();
    f.svc.skin_get(UUID.into()).await.unwrap();
    assert_eq!(f.sink.skin_updates(), before, "same skin: no new event");

    // The services fail: the cached skin stays and is still served.
    {
        let mut m = f.mock.lock().unwrap();
        m.fail_profile = true;
        m.fail_session = true;
        m.skin_ver = 5;
    }
    let cache = f.paths().cache().join("skins").join(format!("{UUID}.json"));
    let cached = std::fs::read(&cache).unwrap();
    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    assert_eq!(info.source, SkinSourceName::Account);
    assert_eq!(
        std::fs::read(&cache).unwrap(),
        cached,
        "not replaced by Steve"
    );
    assert_eq!(f.sink.skin_updates(), before);
}

#[tokio::test]
async fn startup_prefetch_uses_the_session_server_without_tokens() {
    let f = fx(false, None).await;
    let cache = f.paths().cache().join("skins").join(format!("{UUID}.json"));
    std::fs::remove_file(&cache).unwrap();
    let token_calls = f.calls("POST", "/live/token");
    let profile_calls = f.calls("GET", "/mc/profile");
    let (svc, sink, _) = open_svc(f.tmp.path(), &f.ep, f.keys.clone(), false, None);
    let uuid_cache = cache.clone();
    svc.spawn_skin_prefetch();
    wait_for(move || uuid_cache.is_file()).await;
    assert!(sink.skin_updates() >= 1 || cache.is_file());
    assert_eq!(
        f.calls("POST", "/live/token"),
        token_calls,
        "no token refresh at startup"
    );
    assert_eq!(
        f.calls("GET", "/mc/profile"),
        profile_calls,
        "no authenticated read"
    );
    assert!(
        f.seen.lock().unwrap().iter().any(|r| r
            .path
            .starts_with(&format!("/session/{UUID}?unsigned=false"))),
        "session server asked with unsigned=false"
    );
}

// ---------------------------------------------------------------- upload

#[tokio::test]
async fn upload_sends_the_multipart_with_bearer_busts_the_cache_and_leaks_nothing() {
    let f = fx(false, None).await;
    let head_before = f.head_url();
    let events_before = f.sink.skin_updates();
    let file = f.write_png("my skin.png", &skin_png(64, 64, 42));

    let info = f
        .svc
        .skin_upload(UUID.into(), file, SkinVariantName::Slim)
        .await
        .unwrap();
    let up = f.mock.lock().unwrap().uploads.clone();
    assert_eq!(up.len(), 1);
    let (head, body) = &up[0];
    assert!(
        head.to_ascii_lowercase()
            .contains("authorization: bearer mc-token"),
        "bearer sent"
    );
    assert!(head
        .to_ascii_lowercase()
        .contains("multipart/form-data; boundary="));
    assert!(contains(body, b"name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(contains(body, b"name=\"file\"; filename=\"myskin.png\""));
    assert!(contains(body, b"Content-Type: image/png"));
    assert!(contains(body, &skin_png(64, 64, 42)));

    // Cache refetched, head URL changed, event emitted.
    assert_ne!(f.head_url(), head_before, "cache bust");
    assert!(f.sink.skin_updates() > events_before);
    assert!(info.skin_url.contains("?v="));
    assert_eq!(image_bytes(f.paths(), &info.skin_url), skin_png(64, 64, 2));

    // No token anywhere the UI can see.
    let json = serde_json::to_string(&info).unwrap();
    assert!(!json.contains("mc-token") && !json.contains("rt-1"));
    for e in f.sink.names() {
        assert!(!e.contains("mc-token") && !e.contains("rt-1"), "{e}");
    }
    let accounts = std::fs::read_to_string(f.tmp.path().join("accounts.json")).unwrap();
    assert!(!accounts.contains("mc-token"));
}

#[tokio::test]
async fn upload_validates_paths_files_and_maps_service_errors() {
    let f = fx(false, None).await;
    let up = |path: String| {
        let svc = f.svc.clone();
        async move {
            svc.skin_upload(UUID.into(), path, SkinVariantName::Classic)
                .await
        }
    };
    // Missing file, wrong content, wrong size, too large.
    assert_eq!(
        up("/nonexistent/skin.png".into()).await.unwrap_err().code,
        "skin.not_found"
    );
    let not_png = f.write_png("a.png", b"hello");
    assert_eq!(up(not_png).await.unwrap_err().code, "skin.invalid_png");
    let wrong = f.write_png("b.png", &skin_png(32, 32, 1));
    assert_eq!(up(wrong).await.unwrap_err().code, "skin.invalid_png");
    let mut big = skin_png(64, 64, 1);
    big.resize(1024 * 1024 + 1, 0);
    let big = f.write_png("c.png", &big);
    assert_eq!(up(big).await.unwrap_err().code, "skin.too_large");
    assert!(
        f.mock.lock().unwrap().uploads.is_empty(),
        "nothing was sent"
    );

    // The service refuses: status class in the message, never the token.
    let ok = f.write_png("ok.png", &skin_png(64, 32, 7));
    f.mock.lock().unwrap().upload_status = 400;
    let e = up(ok.clone()).await.unwrap_err();
    assert_eq!(e.code, "skin.upload_failed");
    assert!(
        e.message.contains("4xx") && !e.message.contains("mc-token"),
        "{e:?}"
    );
    assert!(!e.needs_relogin);
    f.mock.lock().unwrap().upload_status = 503;
    assert!(up(ok.clone()).await.unwrap_err().message.contains("5xx"));
    // Unauthorized: sign in again.
    f.mock.lock().unwrap().upload_status = 401;
    let e = up(ok).await.unwrap_err();
    assert!(e.needs_relogin, "{e:?}");
}

#[tokio::test]
async fn strict_paths_require_a_picked_file_for_skin_upload() {
    let tmp_png = tempfile::tempdir().unwrap();
    let pick = tmp_png.path().join("picked.png");
    std::fs::write(&pick, skin_png(64, 64, 3)).unwrap();
    let f = fx(true, Some(pick.clone())).await;
    let path = pick.display().to_string();
    let e = f
        .svc
        .skin_upload(UUID.into(), path.clone(), SkinVariantName::Classic)
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");
    assert_eq!(
        f.svc.pick_image_file(None).await.unwrap().as_deref(),
        Some(path.as_str())
    );
    f.svc
        .skin_upload(UUID.into(), path, SkinVariantName::Classic)
        .await
        .unwrap();
    assert_eq!(f.mock.lock().unwrap().uploads.len(), 1);
    assert!(f.dialog.image.is_some());
}

#[test]
fn skin_png_validation_table() {
    let ok = |b: &[u8]| validate_skin_png(b).map_err(|e| e.code);
    assert_eq!(ok(&skin_png(64, 64, 1)), Ok(()));
    assert_eq!(ok(&skin_png(64, 32, 1)), Ok(()));
    for (w, h) in [(32, 32), (128, 128), (64, 48), (64, 128), (63, 64), (1, 1)] {
        assert_eq!(
            ok(&skin_png(w, h, 1)),
            Err("skin.invalid_png".into()),
            "{w}x{h}"
        );
    }
    assert_eq!(ok(b""), Err("skin.invalid_png".into()));
    assert_eq!(ok(b"GIF89a....."), Err("skin.invalid_png".into()));
    let good = skin_png(64, 64, 1);
    assert_eq!(
        ok(&good[..good.len() / 2]),
        Err("skin.invalid_png".into()),
        "truncated"
    );
    let mut big = good;
    big.resize(1024 * 1024 + 1, 0);
    assert_eq!(ok(&big), Err("skin.too_large".into()));
    // Exactly 1 MB passes the size gate (and then fails the PNG check only
    // if damaged, which padding after IEND does not).
    let mut exact = skin_png(64, 64, 1);
    exact.resize(1024 * 1024, 0);
    assert_eq!(ok(&exact), Ok(()));
}

// ---------------------------------------------------------------- capes

#[tokio::test]
async fn cape_can_be_set_and_cleared() {
    let f = fx(false, None).await;
    let info = f
        .svc
        .skin_set_cape(UUID.into(), Some("cape-2".into()))
        .await
        .unwrap();
    {
        let m = f.mock.lock().unwrap();
        assert_eq!(m.cape_calls.len(), 1);
        assert_eq!(m.cape_calls[0].0, "PUT");
        assert_eq!(
            serde_json::from_str::<Value>(&m.cape_calls[0].1).unwrap(),
            json!({"capeId": "cape-2"})
        );
    }
    assert!(!info.capes[0].active && info.capes[1].active);
    assert_eq!(info.cape_url.as_deref(), Some(info.capes[1].url.as_str()));

    let info = f.svc.skin_set_cape(UUID.into(), None).await.unwrap();
    assert_eq!(f.mock.lock().unwrap().cape_calls[1].0, "DELETE");
    assert!(info.cape_url.is_none() && info.capes.iter().all(|c| !c.active));
    assert!(!f.svc.app_state().unwrap().accounts[0].has_cape);

    for bad in ["", "../x", "a/b", &"x".repeat(65)] {
        assert_eq!(
            f.svc
                .skin_set_cape(UUID.into(), Some(bad.into()))
                .await
                .unwrap_err()
                .code,
            "validation"
        );
    }
}

// ---------------------------------------------------------------- username copy

#[tokio::test]
async fn copying_another_players_skin_uploads_it_with_its_model() {
    let f = fx(false, None).await;
    f.mock.lock().unwrap().other_slim = true;
    let info = f
        .svc
        .skin_use_username(UUID.into(), "Notch".into(), None)
        .await
        .unwrap();
    let up = f.mock.lock().unwrap().uploads.clone();
    assert_eq!(up.len(), 1);
    assert!(contains(&up[0].1, b"name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(contains(&up[0].1, &skin_png(64, 64, 99)));
    assert!(info.can_edit);

    // An explicit variant wins.
    f.svc
        .skin_use_username(UUID.into(), "Notch".into(), Some(SkinVariantName::Classic))
        .await
        .unwrap();
    assert!(contains(
        &f.mock.lock().unwrap().uploads[1].1,
        b"name=\"variant\"\r\n\r\nclassic\r\n"
    ));
}

#[tokio::test]
async fn username_copy_handles_unknown_invalid_and_fallback_lookups() {
    let f = fx(false, None).await;
    let e = f
        .svc
        .skin_use_username(UUID.into(), "Nobody".into(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "skin.not_found");
    for bad in ["ab", "has space", "waytoolongusername1", "a/b", "../x", ""] {
        assert_eq!(
            f.svc
                .skin_use_username(UUID.into(), bad.into(), None)
                .await
                .unwrap_err()
                .code,
            "auth.invalid_username",
            "{bad:?}"
        );
    }
    assert!(f.mock.lock().unwrap().uploads.is_empty());

    // The current lookup is down (503): the older endpoint answers.
    {
        let mut m = f.mock.lock().unwrap();
        m.lookup_status = 503;
        m.lookup_legacy_ok = true;
    }
    f.svc
        .skin_use_username(UUID.into(), "Notch".into(), None)
        .await
        .unwrap();
    assert_eq!(f.mock.lock().unwrap().uploads.len(), 1);
    // Unknown on the older endpoint (204) is "not found", not an error.
    let e = f
        .svc
        .skin_use_username(UUID.into(), "Other_1".into(), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "skin.not_found");
}

// ---------------------------------------------------------------- offline accounts

#[tokio::test]
async fn offline_accounts_cannot_change_skins_and_never_touch_the_network() {
    let mock = Arc::new(Mutex::new(Mock::default()));
    let (base, seen) = serve(mock_handler(mock)).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    let (svc, sink, _) = open_svc(
        tmp.path(),
        &ep,
        Arc::new(MemoryDeviceKey::new()),
        false,
        None,
    );
    let acc = svc.account_add_offline("Alex_1".into()).unwrap();
    let id = acc.uuid.clone();
    let png = tmp.path().join("s.png");
    std::fs::write(&png, skin_png(64, 64, 1)).unwrap();
    let path = png.display().to_string();

    let info = svc.skin_get(id.clone()).await.unwrap();
    assert!(!info.can_edit && info.capes.is_empty() && info.cape_url.is_none());
    assert_eq!(info.source, SkinSourceName::Default);
    let refuse = |e: AppError| assert_eq!(e.code, "skin.offline_account");
    refuse(
        svc.skin_upload(id.clone(), path, SkinVariantName::Classic)
            .await
            .unwrap_err(),
    );
    refuse(
        svc.skin_use_username(id.clone(), "Notch".into(), None)
            .await
            .unwrap_err(),
    );
    refuse(svc.skin_set_cape(id.clone(), None).await.unwrap_err());
    let lib = svc
        .skin_library_add(png.display().to_string(), None, SkinVariantName::Classic)
        .await
        .unwrap();
    refuse(
        svc.skin_library_apply(id.clone(), lib.id)
            .await
            .unwrap_err(),
    );
    assert_eq!(
        svc.skin_get("nope".into()).await.unwrap_err().code,
        "account_not_found"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "no request for offline accounts"
    );
    assert_eq!(sink.skin_updates(), 0);
    // The default skin and head still resolve (bundled Steve).
    let head = svc.app_state().unwrap().accounts[0].head_url.clone();
    assert_eq!(head, format!("mangoimg://head/{id}"));
    assert!(matches!(
        resolve(svc.paths(), &head),
        Ok(Image::Bytes { .. })
    ));
    assert_eq!(
        image_bytes(svc.paths(), &info.skin_url),
        mango_auth::default_skin_png()
    );
}

// ---------------------------------------------------------------- library

#[tokio::test]
async fn library_add_list_save_apply_remove() {
    let f = fx(false, None).await;
    assert!(f.svc.skin_library_list().await.unwrap().is_empty());
    let file = f.write_png("one.png", &skin_png(64, 64, 11));
    let a = f
        .svc
        .skin_library_add(
            file.clone(),
            Some("  My\nSkin  ".into()),
            SkinVariantName::Slim,
        )
        .await
        .unwrap();
    assert_eq!(
        (a.name.as_str(), a.variant),
        ("MySkin", SkinVariantName::Slim)
    );
    assert_eq!(a.url, format!("mangoimg://libskin/{}", a.id));
    assert_eq!(image_bytes(f.paths(), &a.url), skin_png(64, 64, 11));
    // The name falls back to the file name; 48 chars at most.
    let b = f
        .svc
        .skin_library_add(file.clone(), None, SkinVariantName::Classic)
        .await
        .unwrap();
    assert_eq!(b.name, "one");
    let c = f
        .svc
        .skin_library_add(file, Some("n".repeat(80)), SkinVariantName::Classic)
        .await
        .unwrap();
    assert_eq!(c.name.chars().count(), 48);
    let list = f.svc.skin_library_list().await.unwrap();
    assert_eq!(
        list.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        [a.id.clone(), b.id.clone(), c.id.clone()]
    );

    // Save the account's current skin.
    let cur = f
        .svc
        .skin_library_save_current(UUID.into(), None)
        .await
        .unwrap();
    assert_eq!(cur.name, "Steve");
    assert_eq!(image_bytes(f.paths(), &cur.url), skin_png(64, 64, 1));

    // Apply uploads that skin with its variant.
    let info = f
        .svc
        .skin_library_apply(UUID.into(), a.id.clone())
        .await
        .unwrap();
    let up = f.mock.lock().unwrap().uploads.clone();
    assert_eq!(up.len(), 1);
    assert!(contains(&up[0].1, b"name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(contains(&up[0].1, &skin_png(64, 64, 11)));
    assert!(info.can_edit);

    // Remove: file and index entry go; unknown / malformed ids are refused.
    let rest = f.svc.skin_library_remove(b.id.clone()).await.unwrap();
    assert_eq!(rest.len(), 3);
    assert!(!rest.iter().any(|s| s.id == b.id));
    assert!(!crate::skins::library_dir(f.paths())
        .join(format!("{}.png", b.id))
        .exists());
    for bad in [b.id.clone(), "x".into(), "../index".into(), "A".repeat(32)] {
        assert_eq!(
            f.svc.skin_library_remove(bad).await.unwrap_err().code,
            "skin.not_found"
        );
    }
    assert_eq!(
        f.svc
            .skin_library_apply(UUID.into(), "zz".into())
            .await
            .unwrap_err()
            .code,
        "skin.not_found"
    );

    // Survives a restart; the index is plain JSON.
    let (again, _, _) = open_svc(f.tmp.path(), &f.ep, f.keys.clone(), false, None);
    assert_eq!(again.skin_library_list().await.unwrap().len(), 3);
    let idx: Value = serde_json::from_slice(
        &std::fs::read(crate::skins::library_dir(f.paths()).join("index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(idx["version"], 1);
    assert_eq!(idx["skins"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn library_enforces_limits_and_survives_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let (svc, _, _) = open_svc(
        tmp.path(),
        &endpoints("http://127.0.0.1:1"),
        Arc::new(MemoryDeviceKey::new()),
        false,
        None,
    );
    let png = tmp.path().join("s.png");
    std::fs::write(&png, skin_png(64, 64, 5)).unwrap();
    let path = png.display().to_string();
    for i in 0..MAX_LIBRARY_SKINS {
        svc.skin_library_add(
            path.clone(),
            Some(format!("s{i}")),
            SkinVariantName::Classic,
        )
        .await
        .unwrap();
    }
    let e = svc
        .skin_library_add(path.clone(), None, SkinVariantName::Classic)
        .await
        .unwrap_err();
    assert_eq!(e.code, "skin.library_full");
    assert_eq!(
        svc.skin_library_list().await.unwrap().len(),
        MAX_LIBRARY_SKINS
    );
    // Removing one makes room again.
    let first = svc.skin_library_list().await.unwrap()[0].id.clone();
    svc.skin_library_remove(first).await.unwrap();
    svc.skin_library_add(path.clone(), None, SkinVariantName::Classic)
        .await
        .unwrap();

    // Oversized and invalid files are refused before anything is stored.
    let big = tmp.path().join("big.png");
    let mut bytes = skin_png(64, 64, 5);
    bytes.resize(1024 * 1024 + 1, 0);
    std::fs::write(&big, bytes).unwrap();
    assert_eq!(
        svc.skin_library_add(big.display().to_string(), None, SkinVariantName::Classic)
            .await
            .unwrap_err()
            .code,
        "skin.too_large"
    );

    // A corrupt index is set aside and the library restarts empty.
    let dir = crate::skins::library_dir(svc.paths());
    std::fs::write(dir.join("index.json"), b"{not json").unwrap();
    assert!(svc.skin_library_list().await.unwrap().is_empty());
    assert!(dir.join("index.json.corrupt").is_file());
    svc.skin_library_add(path, None, SkinVariantName::Classic)
        .await
        .unwrap();
    assert_eq!(svc.skin_library_list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn library_add_needs_a_granted_path_when_strict() {
    let tmp = tempfile::tempdir().unwrap();
    let png = tmp.path().join("s.png");
    std::fs::write(&png, skin_png(64, 64, 5)).unwrap();
    let (svc, _, _) = open_svc(
        tmp.path(),
        &endpoints("http://127.0.0.1:1"),
        Arc::new(MemoryDeviceKey::new()),
        true,
        Some(png.clone()),
    );
    let path = png.display().to_string();
    let e = svc
        .skin_library_add(path.clone(), None, SkinVariantName::Classic)
        .await
        .unwrap_err();
    assert_eq!(e.code, "path.not_granted");
    svc.pick_image_file(None).await.unwrap();
    svc.skin_library_add(path, None, SkinVariantName::Classic)
        .await
        .unwrap();
}

// ---------------------------------------------------------------- protocol

#[tokio::test]
async fn skin_routes_accept_and_reject_a_strict_table() {
    let f = fx(false, None).await;
    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    let file = f.write_png("lib.png", &skin_png(64, 64, 8));
    let lib = f
        .svc
        .skin_library_add(file, None, SkinVariantName::Classic)
        .await
        .unwrap();
    let p = f.paths();

    for url in [
        format!("mangoimg://skin/{UUID}"),
        format!("mangoimg://skin/{UUID}?v=abc123"),
        format!("mangoimg://head/{UUID}?v=ABC9"),
        format!("mangoimg://cape/{UUID}/cape-1"),
        format!("mangoimg://cape/{UUID}/cape-2?v=1f"),
        format!("http://mangoimg.localhost/skin/{UUID}?v=7"),
        lib.url.clone(),
        info.skin_url.clone(),
        info.capes[0].url.clone(),
    ] {
        assert!(resolve(p, &url).is_ok(), "should resolve: {url}");
    }
    // An unknown skin still serves the default.
    assert!(matches!(
        resolve(p, "mangoimg://skin/ffffffffffffffffffffffffffffffff"),
        Ok(Image::Bytes { .. })
    ));

    let bad_url = [
        "mangoimg://skin/../x".to_string(),
        "mangoimg://skin/zz".to_string(),
        format!("mangoimg://skin/{UUID}?v="),
        format!("mangoimg://skin/{UUID}?x=1"),
        format!("mangoimg://skin/{UUID}?v=a/b"),
        format!("mangoimg://skin/{UUID}?v=a&v=b"),
        format!("mangoimg://skin/{UUID}?v={}", "a".repeat(41)),
        format!("mangoimg://skin/{UUID}#frag"),
        format!("mangoimg://cape/{UUID}"),
        format!("mangoimg://cape/{UUID}/../x"),
        format!("mangoimg://cape/{UUID}/a%2fb"),
        format!("mangoimg://cape/{UUID}/cape-1/extra"),
        "mangoimg://cape/zz/cape-1".to_string(),
        "mangoimg://libskin/zz".to_string(),
        "mangoimg://libskin/../index".to_string(),
        format!("mangoimg://libskin/{}", "A".repeat(32)),
        format!("mangoimg://libskin/{}.png", lib.id),
        // A query is only for account textures.
        format!("mangoimg://libskin/{}?v=1", lib.id),
        "mangoimg://cover/0123456789abcdef?v=1".to_string(),
        "mangoimg://block/stone/top?v=1".to_string(),
    ];
    for url in &bad_url {
        assert!(
            matches!(resolve(p, url), Err(ImageError::BadUrl)),
            "should be BadUrl: {url} -> {:?}",
            resolve(p, url)
        );
    }
    // Well formed but absent.
    assert_eq!(
        resolve(p, &format!("mangoimg://cape/{UUID}/cape-9")).unwrap_err(),
        ImageError::NotFound
    );
    assert_eq!(
        resolve(p, &format!("mangoimg://libskin/{}", "0".repeat(32))).unwrap_err(),
        ImageError::NotFound
    );
}

#[cfg(unix)]
#[tokio::test]
async fn skin_routes_refuse_files_that_escape_their_roots() {
    use std::os::unix::fs::symlink;
    let f = fx(false, None).await;
    let outside = f.tmp.path().join("outside.png");
    std::fs::write(&outside, skin_png(64, 64, 1)).unwrap();
    let id = "1".repeat(32);
    let lib = crate::skins::library_dir(f.paths());
    std::fs::create_dir_all(&lib).unwrap();
    symlink(&outside, lib.join(format!("{id}.png"))).unwrap();
    assert_eq!(
        resolve(f.paths(), &format!("mangoimg://libskin/{id}")).unwrap_err(),
        ImageError::Forbidden
    );
    let capes = crate::skins::cape_dir(f.paths(), UUID);
    symlink(&outside, capes.join("evil.png")).unwrap();
    assert_eq!(
        resolve(f.paths(), &format!("mangoimg://cape/{UUID}/evil")).unwrap_err(),
        ImageError::Forbidden
    );
}

#[test]
fn alex_follows_javas_parity_rule() {
    // UUID.hashCode(): even = Steve, odd = Alex. Only Steve ships.
    assert!(!crate::image_protocol::is_alex(
        "00000000-0000-0000-0000-000000000000"
    ));
    assert!(crate::image_protocol::is_alex(
        "00000000-0000-0000-0000-000000000001"
    ));
}

// ---------------------------------------------------------------- per-instance java and window

fn jar_free_service() -> (tempfile::TempDir, AppService) {
    let tmp = tempfile::tempdir().unwrap();
    let svc = AppService::new(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    (tmp, svc)
}

#[cfg(unix)]
fn fake_java(path: &Path, major: u32) -> String {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!("#!/bin/sh\necho 'openjdk version \"{major}.0.1\" 2024-01-01' >&2\n"),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.display().to_string()
}

#[tokio::test]
async fn window_settings_are_validated_stored_and_exposed() {
    let (tmp, svc) = jar_free_service();
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    assert_eq!(p.window, mango_core::WindowSettings::default());
    assert_eq!(p.java_path, None);
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(
        v["window"],
        json!({"mode": "default", "width": null, "height": null})
    );
    assert_eq!(v["javaPath"], Value::Null);

    let set = |w: Value| svc.profile_update(p.id.clone(), json!({ "window": w }));
    let v = set(json!({"mode": "size", "width": 1920, "height": 1080}))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(v.window).unwrap(),
        json!({"mode": "size", "width": 1920, "height": 1080})
    );
    let v = set(json!({"mode": "fullscreen"})).await.unwrap();
    assert_eq!(
        serde_json::to_value(v.window).unwrap(),
        json!({"mode": "fullscreen", "width": null, "height": null})
    );
    // Boundaries.
    for (w, h) in [(640, 480), (16384, 16384)] {
        set(json!({"mode": "size", "width": w, "height": h}))
            .await
            .unwrap();
    }
    for bad in [
        json!({"mode": "size", "width": 639, "height": 480}),
        json!({"mode": "size", "width": 640, "height": 479}),
        json!({"mode": "size", "width": 16385, "height": 480}),
        json!({"mode": "size", "width": 640, "height": 16385}),
        json!({"mode": "size", "width": 800}),
        json!({"mode": "size"}),
        json!({"mode": "size", "width": 800.5, "height": 600}),
        json!({"mode": "size", "width": "800", "height": 600}),
        json!({"mode": "default", "width": 800, "height": 600}),
        json!({"mode": "fullscreen", "width": 800, "height": 600}),
        json!({"mode": "maximized"}),
        json!({"mode": 1}),
        json!({"width": 800, "height": 600}),
        json!({"mode": "default", "extra": true}),
        json!("size"),
        json!(5),
    ] {
        assert_eq!(
            set(bad.clone()).await.unwrap_err().code,
            "validation",
            "{bad}"
        );
    }
    // Back to the global values; survives a restart and a duplicate.
    set(json!({"mode": "size", "width": 1000, "height": 700}))
        .await
        .unwrap();
    let dup = svc.profile_duplicate(p.id.clone()).await.unwrap();
    assert_eq!(dup.window.width, Some(1000));
    let again = AppService::new(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let mine = again
        .app_state()
        .unwrap()
        .profiles
        .into_iter()
        .find(|x| x.id == p.id)
        .unwrap();
    assert_eq!(mine.window.height, Some(700));
    let v = svc
        .profile_update(p.id.clone(), json!({"window": null}))
        .await
        .unwrap();
    assert_eq!(v.window, mango_core::WindowSettings::default());
}

#[test]
fn launch_window_follows_the_instance_then_the_global_config() {
    use crate::launch::window_for;
    use mango_core::{Config, Profile, WindowMode, WindowSettings};
    let mut config = Config::with_total_ram(8192);
    config.width = 1280;
    config.height = 720;
    let mut p = Profile::new("id".into(), 0);
    assert_eq!(window_for(&p, &config), (false, 1280, 720));
    config.fullscreen = true;
    assert_eq!(
        window_for(&p, &config),
        (true, 1280, 720),
        "default = global"
    );
    p.window = Some(WindowSettings::default());
    assert_eq!(window_for(&p, &config), (true, 1280, 720));
    p.window = Some(WindowSettings {
        mode: WindowMode::Size,
        width: Some(1600),
        height: Some(900),
    });
    assert_eq!(
        window_for(&p, &config),
        (false, 1600, 900),
        "size beats global fullscreen"
    );
    config.fullscreen = false;
    p.window = Some(WindowSettings {
        mode: WindowMode::Fullscreen,
        width: None,
        height: None,
    });
    assert_eq!(window_for(&p, &config), (true, 1280, 720));
}

#[test]
fn launch_arguments_carry_the_instance_window() {
    use mango_minecraft::{build_launch_command, HostEnv, LaunchAccount, LaunchInput, Preset};
    // Same arguments the launch pipeline builds for (fullscreen, w, h).
    let build = |full: bool, w: u32, h: u32| {
        let input = LaunchInput {
            java_path: "java".into(),
            paths: Paths::new("/nonexistent-mango"),
            version: json!({"id": "1.21.1", "mainClass": "net.minecraft.client.main.Main",
                            "minecraftArguments": ""}),
            classpath: Vec::new(),
            client_jar: PathBuf::new(),
            natives_dir: PathBuf::new(),
            assets_index_id: "x".into(),
            is_legacy_assets: false,
            logging_arg: None,
            game_dir: PathBuf::from("/nonexistent-mango/g"),
            profile_id: "id".into(),
            profile_name: "n".into(),
            ram_mb: 2048,
            preset: Preset::from_name("balanced"),
            global_java_args: String::new(),
            profile_java_args: String::new(),
            fullscreen: full,
            width: w,
            height: h,
            quick_join: None,
            account: LaunchAccount {
                name: "Steve".into(),
                uuid: UUID.into(),
                access_token: "0".into(),
                xuid: None,
                offline: true,
            },
            launcher_version: "t".into(),
            env: HostEnv::current(),
        };
        format!("{:?}", build_launch_command(&input).args)
    };
    let full = build(true, 1600, 900);
    assert!(
        full.contains("--fullscreen") && !full.contains("--width"),
        "{full}"
    );
    let sized = build(false, 1600, 900);
    assert!(
        sized.contains("\"--width\", \"1600\"") && sized.contains("\"--height\", \"900\""),
        "{sized}"
    );
    assert!(!sized.contains("--fullscreen"));
}

#[cfg(unix)]
#[tokio::test]
async fn java_path_is_checked_stored_and_cleared() {
    let (tmp, svc) = jar_free_service();
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    let good = fake_java(&tmp.path().join("jdk/bin/java"), 21);
    let upd = |v: Value| svc.profile_update(p.id.clone(), json!({ "javaPath": v }));
    let v = upd(json!(good)).await.unwrap();
    assert_eq!(v.java_path.as_deref(), Some(good.as_str()));
    assert!(
        upd(json!(good)).await.is_ok(),
        "re-sending an unchanged view value works"
    );

    let not_exec = tmp.path().join("plain.txt");
    std::fs::write(&not_exec, "x").unwrap();
    for bad in [
        json!("relative/java"),
        json!("/nonexistent/bin/java"),
        json!(tmp.path().display().to_string()),
        json!(not_exec.display().to_string()),
    ] {
        assert_eq!(
            upd(bad.clone()).await.unwrap_err().code,
            "java.not_runnable",
            "{bad}"
        );
    }
    assert_eq!(upd(json!(5)).await.unwrap_err().code, "validation");
    // Network shares and device paths are refused before anything is run.
    assert_eq!(
        upd(json!("\\\\server\\share\\java.exe"))
            .await
            .unwrap_err()
            .code,
        "path.unsupported"
    );
    assert_eq!(
        svc.app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == p.id)
            .unwrap()
            .java_path
            .as_deref(),
        Some(good.as_str()),
        "failures change nothing"
    );
    let v = upd(json!("  ")).await.unwrap();
    assert_eq!(v.java_path, None);
    upd(json!(good)).await.unwrap();
    assert_eq!(upd(Value::Null).await.unwrap().java_path, None);
}

#[cfg(unix)]
#[tokio::test]
async fn instance_java_beats_the_global_path_and_never_falls_back() {
    let (tmp, svc) = jar_free_service();
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    let global = fake_java(&tmp.path().join("global/bin/java"), 21);
    let inst = fake_java(&tmp.path().join("inst/bin/java"), 21);
    let old = fake_java(&tmp.path().join("old/bin/java"), 8);
    svc.config_set(json!({"javaPath": global})).unwrap();
    let config = svc.inner.with_store(|s| s.config.clone());

    let r = svc
        .resolve_java_for(&p.id, &config, None, 21)
        .await
        .unwrap();
    assert_eq!(
        (r.path.display().to_string(), r.source),
        (global.clone(), mango_java::JavaSource::Custom)
    );
    let r = svc
        .resolve_java_for(&p.id, &config, Some(&inst), 21)
        .await
        .unwrap();
    assert_eq!(r.path.display().to_string(), inst, "instance path wins");
    // A blank instance value means automatic/global.
    let r = svc
        .resolve_java_for(&p.id, &config, Some(" "), 21)
        .await
        .unwrap();
    assert_eq!(r.path.display().to_string(), global);
    // The instance path is not runnable: error, no silent fallback to the global one.
    let e = svc
        .resolve_java_for(&p.id, &config, Some("/nonexistent/java"), 21)
        .await
        .unwrap_err();
    assert_eq!(e.code, "java.not_runnable");
    // Too old for the game: error as the custom-path rule says.
    let e = svc
        .resolve_java_for(&p.id, &config, Some(&old), 21)
        .await
        .unwrap_err();
    assert_eq!(e.code, "java.too_old");
    // And the pure rule.
    use crate::java_cmds::custom_java_path as pick;
    assert_eq!(pick(Some("/a"), "/b"), Some(PathBuf::from("/a")));
    assert_eq!(pick(None, "/b"), Some(PathBuf::from("/b")));
    assert_eq!(pick(Some(""), "/b"), Some(PathBuf::from("/b")));
    assert_eq!(pick(Some("  "), ""), None);
}

#[cfg(unix)]
#[tokio::test]
async fn java_list_covers_managed_system_and_custom_without_duplicates() {
    let tmp = tempfile::tempdir().unwrap();
    let jvms = tmp.path().join("jvms");
    let sys = fake_java(&jvms.join("temurin-17/bin/java"), 17);
    let svc = AppService::new(
        Paths::new(tmp.path().join("data")),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            java_roots: Some(mango_java::SearchRoots {
                java_home: None,
                path_dirs: Vec::new(),
                jvm_dirs: vec![jvms],
            }),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let managed = fake_java(&svc.paths().runtimes().join("jre-21/bin/java"), 21);
    let custom = fake_java(&tmp.path().join("mine/bin/java"), 11);
    svc.config_set(json!({"javaPath": custom})).unwrap();
    let p = svc
        .profile_create("A".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    // The instance points at the system JVM: listed once, as system.
    svc.profile_update(p.id.clone(), json!({"javaPath": sys}))
        .await
        .unwrap();

    let list = svc.java_list().await.unwrap();
    let got: Vec<_> = list
        .iter()
        .map(|j| (j.major, j.source, j.path.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (21, JavaSourceName::Managed, managed),
            (17, JavaSourceName::System, sys),
            (11, JavaSourceName::Custom, custom),
        ]
    );
    assert_eq!(list[0].vendor.as_deref(), Some("Eclipse Temurin"));
    assert_eq!(
        list[1].vendor.as_deref(),
        Some("Eclipse Temurin"),
        "from the path"
    );
    assert_eq!(list[2].vendor, None);
    assert!(list[0].raw.starts_with("openjdk version"));
    let v = serde_json::to_value(&list[0]).unwrap();
    assert_eq!(v["source"], "managed");
    assert!(v["path"].is_string() && v["major"] == 21);
}

#[cfg(unix)]
#[tokio::test]
async fn java_install_validates_reuses_and_downloads_with_progress() {
    // Invalid majors never reach the network.
    let (_tmp, svc) = jar_free_service();
    for m in [0, 7, 100, 4000] {
        assert_eq!(svc.java_install(m).await.unwrap_err().code, "validation");
    }

    // A managed runtime that is already there is reused.
    let tmp = tempfile::tempdir().unwrap();
    let make = |dir: &Path, base: String| {
        AppService::new(
            Paths::new(dir),
            Arc::new(Sink::default()),
            ServiceOptions {
                open_folders: false,
                update_dev: true,
                auto_update_check: false,
                partners: Vec::new(),
                adoptium_base: base,
                download_policy: Some(mango_net::HostPolicy::allow_all()),
                ..ServiceOptions::default()
            },
        )
        .unwrap()
    };
    let svc = make(tmp.path(), "http://127.0.0.1:1/never".into());
    fake_java(&svc.paths().runtimes().join("jre-21/bin/java"), 21);
    let info = svc.java_install(21).await.unwrap();
    assert_eq!((info.major, info.source), (21, JavaSourceName::Managed));
    assert_eq!(info.vendor.as_deref(), Some("Eclipse Temurin"));

    // A real download from a mock Adoptium: archive -> runtimes/jre-17.
    let script = b"#!/bin/sh\necho 'openjdk version \"17.0.9\" 2023-10-17' >&2\n".to_vec();
    let archive = crate::tests_p4::zip_bytes(&[("jdk-17/bin/java", script.as_slice())]);
    let (base, _) = serve(move |base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            if r.path.starts_with("/assets/latest/17/hotspot") {
                json_ok(json!([{"binary": {"package": {
                    "name": "jre17.zip", "link": format!("{base}/pkg/jre17.zip")}}}]))
            } else if r.path == "/pkg/jre17.zip" {
                (200, archive.clone())
            } else {
                (404, vec![])
            }
        })
    })
    .await;
    let tmp2 = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let svc = AppService::new(
        Paths::new(tmp2.path()),
        sink.clone(),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            adoptium_base: base,
            download_policy: Some(mango_net::HostPolicy::allow_all()),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let info = svc.java_install(17).await.unwrap();
    assert_eq!((info.major, info.source), (17, JavaSourceName::Managed));
    assert!(info.path.contains("jre-17"));
    let progress: Vec<String> = sink
        .names()
        .into_iter()
        .filter(|n| n.starts_with("java:progress"))
        .collect();
    assert!(!progress.is_empty(), "progress events: {:?}", sink.names());
    assert!(progress.iter().all(|p| p.contains("\"major\":17")));
    // No Adoptium for 99: a coded error, not a panic.
    assert!(svc.java_install(99).await.is_err());
}

// ---------------------------------------------------------------- panorama pool

#[tokio::test]
async fn panorama_pool_lists_extracted_sets_and_extracts_the_rest_in_the_background() {
    let (tmp, svc) = jar_free_service();
    let paths = svc.paths().clone();
    let pano = |n: u8| {
        (
            format!("assets/minecraft/textures/gui/title/background/panorama_{n}.png"),
            crate::icon_images::encode_png(&image::RgbaImage::from_fn(2, 2, |_, _| {
                image::Rgba([n, 1, 2, 255])
            }))
            .unwrap(),
        )
    };
    let jar = |version: &str, with_panorama: bool| {
        let entries: Vec<(String, Vec<u8>)> = if with_panorama {
            (0..6).map(pano).collect()
        } else {
            vec![("README".into(), b"x".to_vec())]
        };
        let refs: Vec<(&str, &[u8])> = entries
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        let dir = paths.versions().join(version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{version}.jar")),
            crate::tests_p4::zip_bytes(&refs),
        )
        .unwrap();
        // The version json marks it as installed.
        std::fs::write(dir.join(format!("{version}.json")), b"{}").unwrap();
    };
    jar("1.20.4", true);
    jar("1.21.1", true);
    jar("1.19.2", false); // no panorama inside: skipped, never an error
                          // Installed but without a client jar (a loader profile): skipped.
    let loader = paths.versions().join("fabric-loader-0.15-1.21.1");
    std::fs::create_dir_all(&loader).unwrap();
    std::fs::write(loader.join("fabric-loader-0.15-1.21.1.json"), b"{}").unwrap();

    // Nothing extracted yet: empty now, sets arrive with panorama:ready.
    assert!(svc.panorama_pool().await.unwrap().is_empty());
    let s = svc.clone();
    let mut sets = Vec::new();
    for _ in 0..200 {
        sets = s.panorama_pool().await.unwrap();
        if sets.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let versions: Vec<_> = sets.iter().map(|s| s.mc_version.as_str()).collect();
    assert_eq!(
        versions,
        ["1.21.1", "1.20.4"],
        "newest first, jarless and panorama-less skipped"
    );
    assert!(sets
        .iter()
        .all(|s| s.source == PanoramaSource::Own && s.faces.len() == 6));
    assert_eq!(sets[0].faces[0], "mangoimg://panorama/1.21.1/0");
    assert!(matches!(
        resolve(&paths, &sets[0].faces[5]),
        Ok(Image::File { .. })
    ));
    drop(tmp);
}

#[tokio::test]
async fn panorama_pool_announces_each_new_set() {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let svc = AppService::new(
        Paths::new(tmp.path()),
        sink.clone(),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let paths = svc.paths().clone();
    let entries: Vec<(String, Vec<u8>)> = (0..6u8)
        .map(|n| {
            (
                format!("assets/minecraft/textures/gui/title/background/panorama_{n}.png"),
                crate::icon_images::encode_png(&image::RgbaImage::from_fn(2, 2, |_, _| {
                    image::Rgba([n, 1, 2, 255])
                }))
                .unwrap(),
            )
        })
        .collect();
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    for v in ["1.21.1", "1.20.4"] {
        let dir = paths.versions().join(v);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{v}.jar")),
            crate::tests_p4::zip_bytes(&refs),
        )
        .unwrap();
        std::fs::write(dir.join(format!("{v}.json")), b"{}").unwrap();
    }
    assert!(svc.panorama_pool().await.unwrap().is_empty());
    let s = sink.clone();
    wait_for(move || {
        s.names()
            .iter()
            .filter(|n| n.starts_with("panorama:ready"))
            .count()
            == 2
    })
    .await;
    assert!(sink
        .names()
        .iter()
        .all(|n| !n.starts_with("panorama:ready") || n.contains("\"profileId\":\"\"")));
    // Everything is extracted: the next call returns both at once and starts nothing.
    assert_eq!(svc.panorama_pool().await.unwrap().len(), 2);
}

// ---------------------------------------------------------------- window commands outside the shell

#[tokio::test]
async fn window_commands_are_unsupported_in_the_dev_server() {
    let (_tmp, svc) = jar_free_service();
    for cmd in [
        "window_minimize",
        "window_toggle_maximize",
        "window_close",
        "window_is_maximized",
        "window_start_drag",
    ] {
        let e = dispatch::invoke(&svc, cmd, Value::Null).await.unwrap_err();
        assert_eq!(e.code, "window.unsupported", "{cmd}");
    }
    let e = dispatch::invoke(
        &svc,
        "window_start_resize",
        json!({"direction": "northeast"}),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "window.unsupported");
    let e = dispatch::invoke(&svc, "window_start_resize", json!({"direction": "up"}))
        .await
        .unwrap_err();
    assert_eq!(e.code, "bad_args");
    for d in [
        ResizeDirection::North,
        ResizeDirection::NorthEast,
        ResizeDirection::SouthWest,
    ] {
        let v: ResizeDirection = serde_json::from_value(json!(d.as_str())).unwrap();
        assert_eq!(v, d);
    }
}

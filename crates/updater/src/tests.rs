#![allow(clippy::unwrap_used)]
use super::*;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}
fn pub_b64(k: &SigningKey) -> String {
    B64.encode(k.verifying_key().to_bytes())
}
fn payload() -> Vec<u8> {
    (0..4096u32).map(|i| (i % 251) as u8).collect()
}
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

struct Server {
    port: u16,
}

impl Server {
    /// Serve `routes(port)` (path -> (status, body)); anything else is 404.
    async fn start(routes: impl FnOnce(u16) -> HashMap<String, (u16, Vec<u8>)>) -> Self {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let routes = std::sync::Arc::new(routes(port));
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    return;
                };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match s.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let head = String::from_utf8_lossy(&buf).to_string();
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let (status, body) = routes
                        .get(&path)
                        .cloned()
                        .unwrap_or((404, b"nope".to_vec()));
                    let resp = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = s.write_all(resp.as_bytes()).await;
                    let _ = s.write_all(&body).await;
                    let _ = s.shutdown().await;
                });
            }
        });
        Self { port }
    }
}

fn manifest_json(port: u16, version: &str, sha: &str, size: u64, sig: &str) -> Vec<u8> {
    serde_json::json!({
        "version": version,
        "pub_date": "2026-01-01T00:00:00Z",
        "notes": "n",
        "platforms": { "linux-x86_64-appimage": {
            "url": format!("http://127.0.0.1:{port}/update.AppImage"),
            "sha256": sha, "size": size, "signature": sig } }
    })
    .to_string()
    .into_bytes()
}

fn routes_for(
    version: &'static str,
    sign_with: SigningKey,
    sha_override: Option<String>,
) -> impl FnOnce(u16) -> HashMap<String, (u16, Vec<u8>)> {
    move |port| {
        let data = payload();
        let sig = B64.encode(sign_with.sign(&data).to_bytes());
        let sha = sha_override.unwrap_or_else(|| sha256_hex(&data));
        let mut m = HashMap::new();
        m.insert(
            "/latest.json".to_string(),
            (
                200,
                manifest_json(port, version, &sha, data.len() as u64, &sig),
            ),
        );
        m.insert("/update.AppImage".to_string(), (200, data));
        m
    }
}

#[derive(Default)]
struct Recorder(Mutex<Vec<PathBuf>>);
impl Installer for Recorder {
    fn stage(&self, f: &Path) -> Result<()> {
        self.0.lock().unwrap().push(f.to_path_buf());
        Ok(())
    }
    fn apply_on_quit(&self) -> Result<()> {
        Ok(())
    }
}

fn appimage() -> Platform {
    Platform {
        os: "linux".into(),
        arch: "x86_64".into(),
        appimage: true,
    }
}

fn updater(
    port: u16,
    dir: &Path,
    current: &str,
    pubkey: String,
    installer: Arc<dyn Installer>,
) -> Updater {
    let mut cfg = UpdaterConfig::new(
        format!("http://127.0.0.1:{port}/latest.json"),
        current.parse().unwrap(),
        dir,
    );
    cfg.public_key_b64 = pubkey;
    cfg.platform = appimage();
    cfg.policy = UrlPolicy::insecure_for_tests(&["127.0.0.1"]);
    cfg.fetch.retries = 0;
    cfg.fetch.timeout = Duration::from_secs(5);
    Updater::new(cfg, installer)
}

use std::sync::Arc;

fn dir_is_empty(d: &Path) -> bool {
    std::fs::read_dir(d).unwrap().next().is_none()
}

fn error_code(st: &UpdateStatus) -> ErrorCode {
    match &st.state {
        UpdateState::Error { code, .. } => *code,
        other => panic!("expected error, got {other:?}"),
    }
}

#[tokio::test]
async fn happy_path_stages_and_signals_relaunch() {
    let k = key(1);
    let srv = Server::start(routes_for("1.4.0", k.clone(), None)).await;
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let u = updater(srv.port, dir.path(), "1.3.4", pub_b64(&k), rec.clone());
    let relaunch = u.subscribe_relaunch();
    let st = u.check(true).await;
    assert_eq!(
        st.state,
        UpdateState::Ready {
            version: "1.4.0".into()
        }
    );
    assert!(st.applying);
    assert!(*relaunch.borrow());
    let staged = rec.0.lock().unwrap().clone();
    assert_eq!(staged.len(), 1);
    assert_eq!(std::fs::read(&staged[0]).unwrap(), payload());
    assert!(u.install().unwrap());
}

#[tokio::test]
async fn offer_only_does_not_signal_relaunch() {
    let k = key(1);
    let srv = Server::start(routes_for("1.4.0", k.clone(), None)).await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.3.4",
        pub_b64(&k),
        Arc::new(NoopInstaller),
    );
    let relaunch = u.subscribe_relaunch();
    assert!(matches!(
        u.check(false).await.state,
        UpdateState::Ready { .. }
    ));
    assert!(!*relaunch.borrow());
}

#[tokio::test]
async fn bad_signature_refused_and_file_deleted() {
    let srv = Server::start(routes_for("1.4.0", key(2), None)).await;
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let u = updater(srv.port, dir.path(), "1.3.4", pub_b64(&key(1)), rec.clone());
    let st = u.check(true).await;
    assert_eq!(error_code(&st), ErrorCode::Other);
    assert!(!st.applying);
    assert!(dir_is_empty(dir.path()));
    assert!(rec.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn placeholder_public_key_fails_closed() {
    let k = key(1);
    let srv = Server::start(routes_for("1.4.0", k, None)).await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.3.4",
        "REPLACE_WITH_BASE64_ED25519_PUBLIC_KEY".into(),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Other);
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn update_signed_by_a_foreign_key_is_refused_with_the_compiled_in_key() {
    let k = key(1);
    let srv = Server::start(routes_for("1.4.0", k, None)).await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.3.4",
        UPDATE_PUBLIC_KEY_B64.into(),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Other);
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn sha256_mismatch_refused() {
    let k = key(1);
    let srv = Server::start(routes_for("1.4.0", k.clone(), Some("ab".repeat(32)))).await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.3.4",
        pub_b64(&k),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Other);
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn downgrade_and_equal_ignored() {
    let k = key(1);
    for v in ["1.2.0", "1.3.4"] {
        let srv = Server::start(routes_for(v, k.clone(), None)).await;
        let dir = tempfile::tempdir().unwrap();
        let u = updater(
            srv.port,
            dir.path(),
            "1.3.4",
            pub_b64(&k),
            Arc::new(NoopInstaller),
        );
        assert_eq!(u.check(true).await.state, UpdateState::Current);
        assert!(dir_is_empty(dir.path()));
    }
}

#[tokio::test]
async fn prerelease_ignored_unless_allowed() {
    assert!(!is_newer(
        &"1.5.0-beta.1".parse().unwrap(),
        &"1.4.0".parse().unwrap(),
        false
    ));
    assert!(is_newer(
        &"1.5.0-beta.1".parse().unwrap(),
        &"1.4.0".parse().unwrap(),
        true
    ));
    assert!(is_newer(
        &"1.5.0".parse().unwrap(),
        &"1.5.0-beta.1".parse().unwrap(),
        false
    ));
    assert!(!is_newer(
        &"1.4.0-rc.1".parse().unwrap(),
        &"1.4.0".parse().unwrap(),
        true
    ));
}

#[tokio::test]
async fn http_manifest_url_rejected_by_default_policy() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = UpdaterConfig::new(
        "http://github.com/x/latest.json",
        "1.0.0".parse().unwrap(),
        dir.path(),
    );
    cfg.platform = appimage();
    let u = Updater::new(cfg, Arc::new(NoopInstaller));
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Other);
}

#[test]
fn manifest_validation_rejects_bad_urls_and_values() {
    let policy = UrlPolicy::default();
    let sha = "ab".repeat(32);
    let sig = B64.encode([0u8; 64]);
    let make = |url: &str, size: u64, sha: &str| {
        serde_json::json!({"version":"1.0.0","platforms":{"windows-x86_64":
            {"url":url,"sha256":sha,"size":size,"signature":sig}}})
        .to_string()
        .into_bytes()
    };
    let good = "https://github.com/o/r/releases/download/v1/a.exe";
    assert!(UpdateManifest::parse(&make(good, 5000, &sha), &policy).is_ok());
    for (url, size, s) in [
        ("http://github.com/o/r/a.exe", 5000, sha.as_str()),
        ("https://evil.example/a.exe", 5000, sha.as_str()),
        ("https://github.com.evil.example/a.exe", 5000, sha.as_str()),
        ("https://user@github.com/a.exe", 5000, sha.as_str()),
        ("https://github.com:8443/a.exe", 5000, sha.as_str()),
        (good, 10, sha.as_str()),
        (good, MAX_UPDATE_SIZE + 1, sha.as_str()),
        (good, 5000, "xyz"),
    ] {
        assert!(
            UpdateManifest::parse(&make(url, size, s), &policy).is_err(),
            "{url} {size} {s}"
        );
    }
    // Missing signature is a hard error.
    let unsigned = br#"{"version":"1.0.0","platforms":{"windows-x86_64":{"url":"https://github.com/a","sha256":"","size":5000}}}"#;
    assert!(UpdateManifest::parse(unsigned, &policy).is_err());
    // Injectable allow-list.
    let mirror = UrlPolicy::with_hosts(["mirror.example".to_string()]);
    assert!(
        UpdateManifest::parse(&make("https://mirror.example/a.exe", 5000, &sha), &mirror).is_ok()
    );
}

#[tokio::test]
async fn foreign_payload_host_in_served_manifest_rejected() {
    let srv = Server::start(|_| {
        let body = serde_json::json!({"version":"2.0.0","platforms":{"linux-x86_64-appimage":
            {"url":"http://evil.example/a","sha256":"ab".repeat(32),"size":5000,
             "signature":B64.encode([0u8;64])}}})
        .to_string()
        .into_bytes();
        HashMap::from([("/latest.json".to_string(), (200, body))])
    })
    .await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.0.0",
        pub_b64(&key(1)),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Other);
}

#[tokio::test]
async fn offline_maps_to_offline() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        port,
        dir.path(),
        "1.0.0",
        pub_b64(&key(1)),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::Offline);
}

#[tokio::test]
async fn missing_manifest_maps_to_no_release() {
    let srv = Server::start(|_| HashMap::new()).await;
    let dir = tempfile::tempdir().unwrap();
    let u = updater(
        srv.port,
        dir.path(),
        "1.0.0",
        pub_b64(&key(1)),
        Arc::new(NoopInstaller),
    );
    assert_eq!(error_code(&u.check(false).await), ErrorCode::NoRelease);
}

#[tokio::test]
async fn disabled_in_dev_and_on_plain_linux() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = UpdaterConfig::new("https://github.com/x", "1.0.0".parse().unwrap(), dir.path());
    cfg.dev = true;
    let u = Updater::new(cfg.clone(), Arc::new(NoopInstaller));
    assert_eq!(
        u.check(true).await.state,
        UpdateState::Disabled {
            reason: DisabledReason::Dev
        }
    );

    cfg.dev = false;
    cfg.platform = Platform {
        os: "linux".into(),
        arch: "x86_64".into(),
        appimage: false,
    };
    let u = Updater::new(cfg, Arc::new(NoopInstaller));
    let st = u.check(true).await;
    assert_eq!(
        st.state,
        UpdateState::Disabled {
            reason: DisabledReason::Unsupported
        }
    );
    assert!(!st.applying);
}

#[test]
fn platform_keys() {
    assert_eq!(
        appimage().manifest_keys(),
        ["linux-x86_64-appimage", "linux-x86_64"]
    );
    let mac = Platform {
        os: "macos".into(),
        arch: "aarch64".into(),
        appimage: false,
    };
    assert_eq!(mac.manifest_keys(), ["darwin-aarch64"]);
    assert!(mac.supported());
}

#[test]
fn state_serializes_like_the_js_states() {
    let st = UpdateStatus {
        state: UpdateState::Error {
            code: ErrorCode::NoRelease,
            message: "m".into(),
        },
        current: "1.0.0".into(),
        applying: false,
    };
    let v = serde_json::to_value(&st).unwrap();
    assert_eq!(v["state"], "error");
    assert_eq!(v["code"], "no-release");
}

#[tokio::test(start_paused = true)]
async fn periodic_ticks_every_three_hours() {
    let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let c = count.clone();
    let h = tokio::spawn(crate::updater::tick_loop(RECHECK_INTERVAL, move || {
        let c = c.clone();
        async move {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }));
    let n = || count.load(std::sync::atomic::Ordering::SeqCst);
    tokio::time::sleep(Duration::from_secs(3 * 3600 - 60)).await;
    assert_eq!(n(), 0);
    tokio::time::sleep(Duration::from_secs(120)).await;
    assert_eq!(n(), 1);
    tokio::time::sleep(Duration::from_secs(3 * 3600)).await;
    assert_eq!(n(), 2);
    h.abort();
}

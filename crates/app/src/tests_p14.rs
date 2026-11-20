#![allow(clippy::unwrap_used)]
//! Skin and cape latency and reliability: stale-while-revalidate, parallel
//! and cached texture fetches, retries, `Retry-After`, timeouts, the cape
//! change answer, single-flight token refresh and the error code table.
//! Mock servers only (with delays, scripted replies and headers).

use crate::service::lock;
use crate::skins::skin_err;
use crate::tests_p13::{cape_png, endpoints, open_svc, skin_png, wait_for, Sink, UUID};
use crate::tests_p2::Req;
use crate::*;
use mango_auth::{AuthError, MemoryDeviceKey};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const BEARER: &str = "mc-token-secret";

#[derive(Clone)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    delay_ms: u64,
}

impl Reply {
    fn json(v: Value) -> Self {
        Reply {
            status: 200,
            headers: Vec::new(),
            body: serde_json::to_vec(&v).unwrap(),
            delay_ms: 0,
        }
    }
    fn status(status: u16) -> Self {
        Reply {
            status,
            headers: Vec::new(),
            body: b"{}".to_vec(),
            delay_ms: 0,
        }
    }
    fn with_header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    fn delayed(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }
}

#[derive(Default)]
struct M {
    counts: HashMap<String, usize>,
    skin_ver: u32,
    active_cape: Option<String>,
    /// Replies for `GET /mc/profile`, consumed first.
    profile_script: VecDeque<Reply>,
    profile_delay: u64,
    tex_delay: u64,
    live_delay: u64,
    live_n: u32,
    session_fails: bool,
    tex_fails: bool,
    /// Overrides the answer of `PUT /mc/capes`.
    put_reply: Option<Value>,
    put_bodies: Vec<(String, String)>,
}

impl M {
    fn count(&self, k: &str) -> usize {
        self.counts.get(k).copied().unwrap_or(0)
    }
}

fn profile_json(base: &str, m: &M) -> Value {
    let cape = |id: &str, alias: &str, n: u8| {
        json!({"id": id, "alias": alias, "url": format!("{base}/tex/cape{n}"),
               "state": if m.active_cape.as_deref() == Some(id) { "ACTIVE" } else { "INACTIVE" }})
    };
    json!({
        "id": UUID, "name": "Steve",
        "skins": [{"state": "ACTIVE", "variant": "CLASSIC",
                   "url": format!("{base}/tex/skin-{}", m.skin_ver)}],
        "capes": [cape("cape-1", "Migrator", 1), cape("cape-2", "Vanilla", 2)],
    })
}

fn handle(base: &str, m: &Arc<Mutex<M>>, r: &Req) -> Reply {
    let mut m = m.lock().unwrap();
    let path = r.path.split('?').next().unwrap_or("").to_string();
    *m.counts.entry(format!("{} {path}", r.method)).or_default() += 1;
    match (r.method.as_str(), path.as_str()) {
        (_, "/live/token") => {
            m.live_n += 1;
            let rt = format!("rt-{}", m.live_n);
            Reply::json(json!({"access_token": "live-at", "refresh_token": rt}))
                .delayed(m.live_delay)
        }
        (_, "/xbl") => Reply::json(json!({"Token": "xbl-token"})),
        (_, "/xsts") => {
            Reply::json(json!({"Token": "xsts", "DisplayClaims": {"xui": [{"uhs": "uhs"}]}}))
        }
        (_, "/mc/login") => Reply::json(json!({"access_token": BEARER, "expires_in": 3600})),
        (_, "/mc/ent") => Reply::json(json!({"items": [{"name": "game_minecraft"}]})),
        ("GET", "/mc/profile") => match m.profile_script.pop_front() {
            Some(r) => r,
            None => Reply::json(profile_json(base, &m)).delayed(m.profile_delay),
        },
        ("PUT", "/mc/capes") => {
            let body = String::from_utf8_lossy(&r.body).to_string();
            let v: Value = serde_json::from_str(&body).unwrap_or_default();
            m.active_cape = v["capeId"].as_str().map(str::to_string);
            m.put_bodies.push((r.head.clone(), body));
            match m.put_reply.clone() {
                Some(v) => Reply::json(v),
                None => Reply::json(profile_json(base, &m)),
            }
        }
        (_, p) if p.starts_with("/tex/") && m.tex_fails => Reply::status(500),
        (_, p) if p.starts_with("/tex/skin-") => Reply {
            body: skin_png(64, 64, m.skin_ver as u8 + 1),
            ..Reply::status(200)
        }
        .delayed(m.tex_delay),
        (_, "/tex/cape1") => Reply {
            body: cape_png(1),
            ..Reply::status(200)
        }
        .delayed(m.tex_delay),
        (_, "/tex/cape2") => Reply {
            body: cape_png(2),
            ..Reply::status(200)
        }
        .delayed(m.tex_delay),
        (_, p) if p.starts_with("/session/") => {
            if m.session_fails {
                return Reply::status(500);
            }
            Reply::status(404)
        }
        _ => Reply::status(404),
    }
}

async fn serve(m: Arc<Mutex<M>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let b = base.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (m, base) = (m.clone(), b.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let head_end = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let len: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap_or(0);
                while buf.len() < head_end + len {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let mut first = head.split_whitespace();
                let req = Req {
                    method: first.next().unwrap_or("").to_string(),
                    path: first.next().unwrap_or("/").to_string(),
                    body: buf[head_end..].to_vec(),
                    head,
                };
                let reply = handle(&base, &m, &req);
                if reply.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(reply.delay_ms)).await;
                }
                let mut out = format!(
                    "HTTP/1.1 {} X\r\nConnection: close\r\nContent-Length: {}\r\n",
                    reply.status,
                    reply.body.len()
                );
                for (k, v) in &reply.headers {
                    out.push_str(&format!("{k}: {v}\r\n"));
                }
                out.push_str("\r\n");
                let mut bytes = out.into_bytes();
                bytes.extend_from_slice(&reply.body);
                let _ = sock.write_all(&bytes).await;
            });
        }
    });
    base
}

struct F {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    m: Arc<Mutex<M>>,
}

/// A service signed in against the mock, its skin cached.
async fn fx(retries: u32, timeout_ms: u64) -> F {
    let m = Arc::new(Mutex::new(M {
        active_cape: Some("cape-1".into()),
        ..M::default()
    }));
    let base = serve(m.clone()).await;
    let ep = mango_auth::Endpoints {
        retries,
        request_timeout: Duration::from_millis(timeout_ms),
        retry_after_cap: Duration::from_secs(3),
        ..endpoints(&base)
    };
    let tmp = tempfile::tempdir().unwrap();
    let (svc, sink, _dialog) = open_svc(
        tmp.path(),
        &ep,
        Arc::new(MemoryDeviceKey::new()),
        false,
        None,
    );
    svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let s = sink.clone();
    wait_for(move || s.skin_updates() >= 1).await;
    F { tmp, svc, sink, m }
}

impl F {
    fn count(&self, k: &str) -> usize {
        self.m.lock().unwrap().count(k)
    }
    fn reset_counts(&self) {
        self.m.lock().unwrap().counts.clear();
    }
    /// Forget the cached skin and capes (the account keeps its stored lists).
    fn drop_cache(&self) {
        let _ = std::fs::remove_dir_all(self.svc.paths().cache().join("skins"));
    }
}

#[tokio::test]
async fn second_skin_get_is_instant_and_an_event_follows_a_changed_skin() {
    let f = fx(0, 5000).await;
    {
        let mut m = f.m.lock().unwrap();
        m.skin_ver = 1;
        m.profile_delay = 400;
    }
    f.reset_counts();
    let before = f.sink.skin_updates();
    let first = f.svc.skin_get(UUID.into()).await.unwrap();
    let t = Instant::now();
    let again = f.svc.skin_get(UUID.into()).await.unwrap();
    assert!(
        t.elapsed() < Duration::from_millis(150),
        "cached answer took {:?}",
        t.elapsed()
    );
    assert!(first.stale && again.stale);
    // The delayed answer arrives later and is announced.
    let s = f.sink.clone();
    wait_for(move || s.skin_updates() > before).await;
    let fresh = f.svc.skin_get(UUID.into()).await.unwrap();
    assert_ne!(
        fresh.skin_url, first.skin_url,
        "the new skin has a new version tag"
    );
    // Only one revalidation ran for the two calls.
    assert_eq!(f.count("GET /mc/profile"), 1);
}

#[tokio::test]
async fn profile_and_textures_load_in_parallel_and_the_disk_cache_prevents_refetches() {
    let f = fx(0, 5000).await;
    f.drop_cache();
    f.reset_counts();
    {
        let mut m = f.m.lock().unwrap();
        m.profile_delay = 300;
        m.tex_delay = 300;
    }
    let t = Instant::now();
    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    let took = t.elapsed();
    // Sequential: profile + skin + 2 capes = 1200 ms; concurrent: about one delay.
    assert!(
        took < Duration::from_millis(750),
        "first load took {took:?}"
    );
    assert!(!info.stale);
    assert_eq!(info.capes.len(), 2);
    assert_eq!(f.count("GET /mc/profile"), 1);
    assert_eq!(f.count("GET /tex/skin-0"), 1);
    assert_eq!(f.count("GET /tex/cape1"), 1);
    assert_eq!(f.count("GET /tex/cape2"), 1);

    // Reopening: cached answer, the background refresh asks the profile only.
    let again = f.svc.skin_get(UUID.into()).await.unwrap();
    assert!(again.stale);
    let probe = f.m.clone();
    wait_for(move || probe.lock().unwrap().count("GET /mc/profile") >= 2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        f.count("GET /tex/skin-0"),
        1,
        "unchanged skin url: no download"
    );
    assert_eq!(f.count("GET /tex/cape1"), 1);
    assert_eq!(f.count("GET /tex/cape2"), 1);
}

#[tokio::test]
async fn a_5xx_is_retried_with_backoff_then_succeeds() {
    let f = fx(2, 5000).await;
    f.drop_cache();
    f.reset_counts();
    {
        let mut m = f.m.lock().unwrap();
        m.profile_script = VecDeque::from([Reply::status(500), Reply::status(503)]);
    }
    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    assert_eq!(info.capes.len(), 2);
    assert_eq!(f.count("GET /mc/profile"), 3);
}

#[tokio::test]
async fn a_429_waits_for_retry_after_and_a_long_demand_is_reported_with_its_seconds() {
    let f = fx(2, 5000).await;
    f.drop_cache();
    f.m.lock().unwrap().profile_script =
        VecDeque::from([Reply::status(429).with_header("Retry-After", "1")]);
    let t = Instant::now();
    let info = f.svc.skin_get(UUID.into()).await.unwrap();
    assert!(
        t.elapsed() >= Duration::from_millis(900),
        "waited {:?}",
        t.elapsed()
    );
    assert_eq!(info.capes.len(), 2);

    // Longer than the cap (3 s here): no waiting, a stable code with the seconds.
    f.drop_cache();
    {
        let mut m = f.m.lock().unwrap();
        m.session_fails = true;
        m.tex_fails = true;
        m.profile_script = VecDeque::from([Reply::status(429).with_header("Retry-After", "30")]);
    }
    let t = Instant::now();
    let err = f.svc.skin_get(UUID.into()).await.unwrap_err();
    assert!(t.elapsed() < Duration::from_millis(800));
    assert_eq!(err.code, "skin.rate_limited");
    assert!(err.message.contains("30"), "{}", err.message);
}

#[tokio::test]
async fn a_hung_service_ends_in_a_timeout_error() {
    let f = fx(0, 250).await;
    f.drop_cache();
    {
        let mut m = f.m.lock().unwrap();
        m.session_fails = true;
        m.tex_fails = true;
        m.profile_script = VecDeque::from([Reply::status(200).delayed(10_000)]);
    }
    let t = Instant::now();
    let err = f.svc.skin_get(UUID.into()).await.unwrap_err();
    assert!(
        t.elapsed() < Duration::from_secs(3),
        "took {:?}",
        t.elapsed()
    );
    assert_eq!(err.code, "skin.timeout");
}

#[tokio::test]
async fn setting_a_cape_uses_the_answer_without_a_second_request() {
    let f = fx(0, 5000).await;
    f.reset_counts();
    let before = f.sink.skin_updates();
    let info = f
        .svc
        .skin_set_cape(UUID.into(), Some("cape-2".into()))
        .await
        .unwrap();
    assert!(!info.capes[0].active && info.capes[1].active);
    assert!(!info.stale);
    assert_eq!(f.count("PUT /mc/capes"), 1);
    assert_eq!(f.count("GET /mc/profile"), 0, "no refetch after the change");
    assert_eq!(f.sink.skin_updates(), before + 1);
    let m = f.m.lock().unwrap();
    let (head, body) = &m.put_bodies[0];
    assert_eq!(
        serde_json::from_str::<Value>(body).unwrap(),
        json!({"capeId": "cape-2"})
    );
    assert_eq!(body, r#"{"capeId":"cape-2"}"#);
    assert!(head
        .to_ascii_lowercase()
        .contains(&format!("authorization: bearer {BEARER}")));
}

#[tokio::test]
async fn an_answer_without_the_cape_active_is_an_error_and_leaks_no_token() {
    let f = fx(0, 5000).await;
    // The service says 200 but keeps the old cape.
    let base_profile = {
        let m = f.m.lock().unwrap();
        profile_json(
            "http://127.0.0.1:1",
            &M {
                active_cape: Some("cape-1".into()),
                skin_ver: m.skin_ver,
                ..M::default()
            },
        )
    };
    f.m.lock().unwrap().put_reply = Some(base_profile);
    f.reset_counts();
    let before = f.sink.skin_updates();
    let err = f
        .svc
        .skin_set_cape(UUID.into(), Some("cape-2".into()))
        .await
        .unwrap_err();
    assert_eq!(err.code, "skin.cape_not_applied");
    let text = format!("{err:?} {}", err.message);
    assert!(!text.contains(BEARER) && !text.contains("rt-"));
    assert_eq!(f.sink.skin_updates(), before, "nothing was announced");
    assert_eq!(f.count("GET /mc/profile"), 0);
    let shown = f.svc.skin_get(UUID.into()).await.unwrap();
    assert!(shown.capes[0].active, "the cached state is untouched");
}

#[tokio::test]
async fn concurrent_calls_refresh_the_token_once_and_save_it_once() {
    let f = fx(0, 5000).await;
    f.m.lock().unwrap().live_delay = 150;
    f.reset_counts();
    {
        // The access token ran out.
        let mut sessions = lock(&f.svc.inner.sessions);
        let s = sessions.get_mut(UUID).unwrap();
        s.account.expires_at = Some(1);
    }
    let changed = |sink: &Sink| {
        sink.0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.name() == "accounts:changed")
            .count()
    };
    let changes_before = changed(&f.sink);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let svc = f.svc.clone();
        set.spawn(async move { svc.refresh_session(UUID, false).await.map(|a| a.expires_at) });
    }
    while let Some(r) = set.join_next().await {
        assert!(r.unwrap().is_ok());
    }
    assert_eq!(
        f.count("POST /live/token"),
        1,
        "one refresh for eight callers"
    );
    assert_eq!(changed(&f.sink) - changes_before, 1, "one save");
    let sessions = lock(&f.svc.inner.sessions);
    let s = sessions.get(UUID).unwrap();
    assert_eq!(
        s.account.msa_refresh_token.as_deref(),
        Some("rt-2"),
        "the rotated token is the one kept"
    );
    assert!(s.persisted);
    drop(sessions);
    let _ = f.tmp.path();
}

#[test]
fn skin_error_code_table() {
    let table: Vec<(AuthError, &str)> = vec![
        (AuthError::OfflineSkin, "skin.offline_account"),
        (AuthError::SkinUpload(500), "skin.upload_failed"),
        (AuthError::RateLimited(7), "skin.rate_limited"),
        (AuthError::CapeNotApplied, "skin.cape_not_applied"),
        (AuthError::TextureHost, "skin.texture_host"),
        (AuthError::Timeout, "skin.timeout"),
        (AuthError::NeedsRelogin, "auth.needs_relogin"),
        (AuthError::NoProfile, "auth.no_profile"),
    ];
    for (e, code) in table {
        assert_eq!(skin_err(e).code, code);
    }
    assert!(skin_err(AuthError::RateLimited(7)).message.contains('7'));
    assert!(skin_err(AuthError::NeedsRelogin).needs_relogin);
}

#![allow(clippy::unwrap_used)]
//! Part 3 of `docs/ipc.md`: Microsoft login, credential persistence through
//! the vault and token refresh. Mock servers and an in-memory device key
//! only: no real keychain, no real Microsoft endpoint.

use crate::tests_p2::{json_ok, serve, Handler, Req};
use crate::*;
use mango_auth::{DeviceKeyStore, MemoryDeviceKey};
use mango_core::Paths;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const UUID: &str = "0123456789abcdef0123456789abcdef";

#[derive(Default)]
struct Sink(Mutex<Vec<Event>>);

impl EventSink for Sink {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl Sink {
    fn changed(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, Event::AccountsChanged(c) if c.uuid == UUID))
            .count()
    }
}

#[derive(Clone)]
struct Chain {
    xsts: (u16, Value),
    ent: (u16, Value),
    profile: (u16, Value),
    /// Forces the Live token endpoint's answer.
    token: Option<(u16, Value)>,
}

fn ok_chain() -> Chain {
    Chain {
        xsts: (
            200,
            json!({"Token": "xsts-token", "DisplayClaims": {"xui": [{"uhs": "uhs123"}]}}),
        ),
        ent: (200, json!({"items": [{"name": "game_minecraft"}]})),
        profile: (
            200,
            json!({"id": UUID, "name": "Steve", "skins": [],
                   "capes": [{"state": "ACTIVE", "url": "http://x/cape"}]}),
        ),
        token: None,
    }
}

/// Mock Live/Xbox/Minecraft services. The token endpoint rotates the refresh
/// token: `rt-1`, `rt-2`, ...
fn handler(chain: Chain) -> impl FnOnce(&str) -> Handler {
    let n = Arc::new(AtomicUsize::new(0));
    move |_base| {
        Arc::new(move |r: &Req| match r.path.as_str() {
            "/live/token" => {
                if let Some((status, v)) = &chain.token {
                    return (*status, serde_json::to_vec(v).unwrap());
                }
                let i = n.fetch_add(1, Ordering::SeqCst) + 1;
                json_ok(json!({"access_token": format!("live-at-{i}"),
                               "refresh_token": format!("rt-{i}")}))
            }
            "/xbl" => json_ok(json!({"Token": "xbl-token"})),
            "/xsts" => (chain.xsts.0, serde_json::to_vec(&chain.xsts.1).unwrap()),
            "/mc/login" => json_ok(json!({"access_token": "mc-token", "expires_in": 3600})),
            "/mc/ent" => (chain.ent.0, serde_json::to_vec(&chain.ent.1).unwrap()),
            "/mc/profile" => (
                chain.profile.0,
                serde_json::to_vec(&chain.profile.1).unwrap(),
            ),
            "/manifest" => (404, vec![]),
            _ => (404, vec![]),
        })
    }
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
        session_profile: format!("{base}/session"),
        retries: 0,
        backoff: Duration::from_millis(1),
        ..mango_auth::Endpoints::default()
    }
}

fn redirect(ep: &mango_auth::Endpoints, query: &str) -> LoginOutcome {
    // `{state}` is filled in by the canned window from the authorize URL.
    LoginOutcome::Code(format!("{}?{query}&state={{state}}", ep.redirect_uri))
}

struct Fx {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    keys: Arc<MemoryDeviceKey>,
    login: Arc<CannedLoginWindow>,
    ep: mango_auth::Endpoints,
    seen: Arc<Mutex<Vec<Req>>>,
}

fn open(
    dir: &std::path::Path,
    ep: &mango_auth::Endpoints,
    keys: Arc<MemoryDeviceKey>,
    outcome: LoginOutcome,
) -> (AppService, Arc<Sink>, Arc<CannedLoginWindow>) {
    let sink = Arc::new(Sink::default());
    let login = Arc::new(CannedLoginWindow::new(outcome));
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        partners: Vec::new(),
        auth_endpoints: ep.clone(),
        manifest_url: ep.live_token.replace("/live/token", "/manifest"),
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        ..ServiceOptions::default()
    };
    let host = Host::system(false)
        .with_login(login.clone())
        .with_device_keys(keys);
    let svc = AppService::with_host(Paths::new(dir), sink.clone(), opts, host).unwrap();
    (svc, sink, login)
}

async fn fx(chain: Chain, outcome: impl FnOnce(&mango_auth::Endpoints) -> LoginOutcome) -> Fx {
    let (base, seen) = serve(handler(chain)).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    let keys = Arc::new(MemoryDeviceKey::new());
    let (svc, sink, login) = open(tmp.path(), &ep, keys.clone(), outcome(&ep));
    Fx {
        tmp,
        svc,
        sink,
        keys,
        login,
        ep,
        seen,
    }
}

impl Fx {
    fn file(&self) -> String {
        // No profile and no account: the file is never written.
        std::fs::read_to_string(self.tmp.path().join("accounts.json")).unwrap_or_default()
    }

    fn account_json(&self, uuid: &str) -> Value {
        let v: Value = serde_json::from_str(&self.file()).unwrap();
        v["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["uuid"] == uuid)
            .cloned()
            .unwrap()
    }

    fn calls(&self, path: &str) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == path)
            .map(|r| String::from_utf8_lossy(&r.body).to_string())
            .collect()
    }

    fn reopen(&self, keys: Arc<MemoryDeviceKey>) -> (AppService, Arc<Sink>) {
        let (svc, sink, _) = open(
            self.tmp.path(),
            &self.ep,
            keys,
            LoginOutcome::Error("unused".into()),
        );
        (svc, sink)
    }
}

fn assert_no_secrets(text: &str) {
    for secret in ["live-at", "rt-1", "rt-2", "mc-token", "accessToken"] {
        assert!(!text.contains(secret), "{secret} leaked into accounts.json");
    }
    assert!(!text.contains("msaRefreshToken"));
}

fn code(e: AppError) -> String {
    e.code
}

// ---------- sign-in ----------

#[tokio::test]
async fn sign_in_persists_through_the_vault_and_never_writes_plaintext() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=the-code")).await;
    let a = f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    assert_eq!(a.uuid, UUID);
    assert_eq!(a.name, "Steve");
    assert!(a.has_cape);
    assert!(a.credentials_persisted && !a.credentials_locked && !a.needs_relogin);
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["credentialsPersisted"], true);
    assert_eq!(v["credentialsLocked"], false);
    assert_eq!(v["needsRelogin"], false);
    assert!(v.get("accessToken").is_none());

    // The window was asked for the authorize page with the redirect prefix.
    let reqs = f.login.requests();
    assert_eq!(reqs.len(), 1);
    assert!(reqs[0].0.contains("response_type=code"));
    assert_eq!(reqs[0].1, f.ep.redirect_uri);
    assert!(f.calls("/live/token")[0].contains("code=the-code"));

    // Envelope only, no plaintext anywhere in the file.
    let text = f.file();
    assert_no_secrets(&text);
    let env = f.account_json(UUID)["encryptedCredentials"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(env.starts_with("mc2:"));
    assert!(!env.contains("rt-1"));

    // Selected and visible in the app state, event emitted.
    let st = f.svc.app_state().unwrap();
    assert_eq!(st.config.selected_account.as_deref(), Some(UUID));
    assert_eq!(st.accounts.len(), 1);
    assert!(st.accounts[0].credentials_persisted);
    assert_eq!(f.sink.changed(), 1);

    // A restart with the same device key keeps the login: the refresh token
    // (not the access token) comes back from the envelope.
    let (again, _) = f.reopen(f.keys.clone());
    let st = again.app_state().unwrap();
    assert!(st.accounts[0].credentials_persisted && !st.accounts[0].needs_relogin);
    let creds = again.launch_credentials(UUID).await.unwrap();
    assert_eq!(creds.access_token(), "mc-token");
    assert!(!creds.offline);
    let refresh_calls = f.calls("/live/token");
    assert!(refresh_calls[1].contains("grant_type=refresh_token"));
    assert!(refresh_calls[1].contains("refresh_token=rt-1"));
}

#[tokio::test]
async fn cancel_and_denied_sign_ins_return_null() {
    let f = fx(ok_chain(), |_| LoginOutcome::Cancelled).await;
    assert_eq!(f.svc.account_sign_in_microsoft().await.unwrap(), None);
    assert!(f.svc.app_state().unwrap().accounts.is_empty());
    assert!(f.calls("/live/token").is_empty());

    let f = fx(ok_chain(), |ep| {
        redirect(ep, "error=access_denied&error_description=nope")
    })
    .await;
    assert_eq!(f.svc.account_sign_in_microsoft().await.unwrap(), None);
    assert!(f.calls("/live/token").is_empty());
    assert_eq!(f.sink.changed(), 0);
}

#[tokio::test]
async fn window_and_redirect_errors_are_login_failed() {
    let f = fx(ok_chain(), |_| LoginOutcome::Error("no window".into())).await;
    let e = f.svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(e.code, "auth.login_failed");
    assert!(!e.needs_relogin);

    let f = fx(ok_chain(), |ep| {
        redirect(ep, "error=server_error&error_description=Too+bad")
    })
    .await;
    let e = f.svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.message.as_str()),
        ("auth.login_failed", "Too bad")
    );

    // A code whose `state` is not the one of this attempt is refused and
    // never redeemed.
    let f = fx(ok_chain(), |ep| {
        LoginOutcome::Code(format!("{}?code=c&state=forged", ep.redirect_uri))
    })
    .await;
    let e = f.svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(e.code, "auth.login_failed");
    assert!(f.calls("/live/token").is_empty());
    let (url, _) = &f.login.requests()[0];
    assert!(url.contains("state="), "{url}");

    // A foreign URL is never treated as a code.
    let f = fx(ok_chain(), |_| {
        LoginOutcome::Code("https://evil.example/?code=x".into())
    })
    .await;
    let e = f.svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(e.code, "auth.login_failed");
    assert!(f.calls("/live/token").is_empty());

    // The no-window default of a shell without a sign-in window.
    let tmp = tempfile::tempdir().unwrap();
    let svc = AppService::with_host(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions::default(),
        Host::system(false),
    )
    .unwrap();
    assert_eq!(
        code(svc.account_sign_in_microsoft().await.unwrap_err()),
        "auth.login_failed"
    );
    // The guard is released after a failure.
    assert_eq!(
        code(svc.account_sign_in_microsoft().await.unwrap_err()),
        "auth.login_failed"
    );
}

#[tokio::test]
async fn every_xbox_and_profile_failure_keeps_its_stable_code() {
    let cases: Vec<(Chain, &str)> = [
        (2148916233_u64, "auth.xbox.no_profile"),
        (2148916235, "auth.xbox.country"),
        (2148916236, "auth.xbox.adult_verification"),
        (2148916237, "auth.xbox.adult_verification"),
        (2148916238, "auth.xbox.child_account"),
    ]
    .into_iter()
    .map(|(x, c)| {
        let mut ch = ok_chain();
        ch.xsts = (401, json!({"XErr": x}));
        (ch, c)
    })
    .chain({
        let mut owns_nothing = ok_chain();
        owns_nothing.profile = (404, json!({}));
        owns_nothing.ent = (200, json!({"items": []}));
        let mut no_profile = ok_chain();
        no_profile.profile = (404, json!({}));
        let mut http = ok_chain();
        http.profile = (403, json!({}));
        [
            (owns_nothing, "auth.not_owned"),
            (no_profile, "auth.no_profile"),
            (http, "auth.profile_http"),
        ]
    })
    .collect();
    for (chain, want) in cases {
        let f = fx(chain, |ep| redirect(ep, "code=c")).await;
        let e = f.svc.account_sign_in_microsoft().await.unwrap_err();
        assert_eq!(e.code, want);
        assert!(!e.needs_relogin);
        // A failed sign-in stores nothing.
        assert!(f.svc.app_state().unwrap().accounts.is_empty());
        assert_no_secrets(&f.file());
    }
}

#[tokio::test]
async fn unreachable_services_report_offline() {
    let tmp = tempfile::tempdir().unwrap();
    // Nothing listens on port 1.
    let ep = endpoints("http://127.0.0.1:1");
    let (svc, _, _) = open(
        tmp.path(),
        &ep,
        Arc::new(MemoryDeviceKey::new()),
        redirect(&ep, "code=c"),
    );
    let e = svc.account_sign_in_microsoft().await.unwrap_err();
    assert_eq!(e.code, "auth.offline");
    assert!(!e.needs_relogin);
}

// ---------- refresh ----------

#[tokio::test]
async fn refresh_rotates_and_persists_the_new_refresh_token() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let env1 = f.account_json(UUID)["encryptedCredentials"].clone();
    let before = f.sink.changed();

    // A still-valid token is not refreshed on launch...
    f.svc.launch_credentials(UUID).await.unwrap();
    assert_eq!(f.calls("/live/token").len(), 1);

    // ...but `account_refresh` forces it, rotating rt-1 -> rt-2.
    let a = f.svc.account_refresh(UUID.into()).await.unwrap();
    assert!(a.credentials_persisted && !a.needs_relogin);
    let calls = f.calls("/live/token");
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("refresh_token=rt-1"));
    assert!(f.sink.changed() > before);
    let env2 = f.account_json(UUID)["encryptedCredentials"].clone();
    assert_ne!(env1, env2);
    assert_no_secrets(&f.file());

    // After a restart the NEW token is used.
    let (again, _) = f.reopen(f.keys.clone());
    again.launch_credentials(UUID).await.unwrap();
    let calls = f.calls("/live/token");
    assert_eq!(calls.len(), 3);
    assert!(calls[2].contains("refresh_token=rt-2"));
    assert_no_secrets(&f.file());
}

#[tokio::test]
async fn rejected_refresh_token_needs_relogin() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    // New process, Microsoft now rejects the token.
    let mut rejected = ok_chain();
    rejected.token = Some((400, json!({"error": "invalid_grant"})));
    let (base, _) = serve(handler(rejected)).await;
    let ep = endpoints(&base);
    let (svc, sink, _) = open(f.tmp.path(), &ep, f.keys.clone(), LoginOutcome::Cancelled);
    let e = svc.account_refresh(UUID.into()).await.unwrap_err();
    assert_eq!(e.code, "auth.needs_relogin");
    assert!(e.needs_relogin);
    assert_eq!(serde_json::to_value(&e).unwrap()["needsRelogin"], true);
    assert_eq!(sink.changed(), 1);
    let st = svc.app_state().unwrap();
    assert!(st.accounts[0].needs_relogin);
    // The envelope is kept; the UI offers a new sign-in.
    assert!(f.account_json(UUID)["encryptedCredentials"].is_string());

    // Launching surfaces the same error before anything else happens.
    let p = svc
        .profile_create("P".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    let e = svc.game_launch(p.id.clone(), None).await.unwrap_err();
    assert!(e.needs_relogin);
    assert!(!svc.is_busy(&p.id));
}

#[tokio::test]
async fn account_without_refresh_token_needs_relogin_without_network() {
    let f = fx(ok_chain(), |_| LoginOutcome::Cancelled).await;
    let raw: mango_core::Account = serde_json::from_value(
        json!({"type": "microsoft", "uuid": UUID, "name": "Steve", "skins": [], "capes": []}),
    )
    .unwrap();
    f.svc.inner.with_store(|s| s.upsert_account(raw)).unwrap();
    let st = f.svc.app_state().unwrap();
    assert!(st.accounts[0].needs_relogin);
    assert!(!st.accounts[0].credentials_persisted);
    let e = f.svc.account_refresh(UUID.into()).await.unwrap_err();
    assert!(e.needs_relogin);
    assert!(f.seen.lock().unwrap().is_empty());
    assert_eq!(
        code(f.svc.account_refresh("nope".into()).await.unwrap_err()),
        "account_not_found"
    );
}

// ---------- launch ----------

#[tokio::test]
async fn game_launch_refreshes_a_microsoft_account_before_the_pipeline_continues() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let (svc, _) = f.reopen(f.keys.clone());
    svc.config_set(json!({"performanceMods": false})).unwrap();
    let p = svc
        .profile_create("P".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    // The version manifest mock answers 404, so the pipeline stops right
    // after the account stage: with a launch error, not `not_implemented`
    // and not an auth error.
    let e = svc.game_launch(p.id.clone(), None).await.unwrap_err();
    assert!(!e.code.starts_with("auth."), "{}", e.code);
    assert_ne!(e.code, "not_implemented");
    let calls = f.calls("/live/token");
    assert_eq!(calls.len(), 2, "one refresh before the pipeline");
    assert!(calls[1].contains("refresh_token=rt-1"));
    assert_eq!(f.calls("/mc/login").len(), 2);
    // The rotated token was persisted.
    let (again, _) = f.reopen(f.keys.clone());
    again.launch_credentials(UUID).await.unwrap();
    // (still valid: persisted state did not break the envelope)
    assert!(again.app_state().unwrap().accounts[0].credentials_persisted);
    assert_no_secrets(&f.file());
}

#[tokio::test]
async fn launch_credentials_are_redacted_and_carry_the_session() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let creds = f.svc.launch_credentials(UUID).await.unwrap();
    let dbg = format!("{creds:?}");
    assert!(!dbg.contains("mc-token") && dbg.contains("redacted"));
    let la = creds.to_launch_account();
    assert!(!la.offline);
    assert_eq!((la.name.as_str(), la.uuid.as_str()), ("Steve", UUID));
    assert_eq!(la.access_token, "mc-token");
    // Offline: the legacy "0" token and offline flag.
    let o = f.svc.account_add_offline("Player_1".into()).unwrap();
    let c = f.svc.launch_credentials(&o.uuid).await.unwrap();
    assert!(c.offline);
    assert_eq!(c.access_token(), "0");
}

// ---------- persistence and migration ----------

#[tokio::test]
async fn offline_accounts_work_unchanged_through_the_vault() {
    let f = fx(ok_chain(), |_| LoginOutcome::Cancelled).await;
    let a = f.svc.account_add_offline("Steve_1".into()).unwrap();
    assert!(a.credentials_persisted && !a.credentials_locked && !a.needs_relogin);
    let entry = f.account_json(&a.uuid);
    assert_eq!(entry["type"], "offline");
    assert!(entry.get("accessToken").is_none());
    assert!(entry.get("encryptedCredentials").is_none());
    let (again, _) = f.reopen(f.keys.clone());
    let st = again.app_state().unwrap();
    assert_eq!(st.accounts[0].name, "Steve_1");
    assert!(st.accounts[0].credentials_persisted);
    assert_eq!(
        again
            .launch_credentials(&a.uuid)
            .await
            .unwrap()
            .access_token(),
        "0"
    );
    assert!(f.seen.lock().unwrap().is_empty());
    again.account_remove(a.uuid.clone()).unwrap();
    assert!(again.app_state().unwrap().accounts.is_empty());
}

fn legacy_file(dir: &std::path::Path) {
    let raw = json!({
        "accounts": [
            {"type": "microsoft", "uuid": UUID, "name": "Steve",
             "accessToken": "legacy-access-SECRET", "msaRefreshToken": "rt-legacy",
             "expiresAt": 1, "skins": [], "capes": [], "keepMe": 7},
            {"type": "offline", "uuid": "11111111111111111111111111111111",
             "name": "Off_1", "accessToken": "0", "skins": [], "capes": []}
        ],
        "profiles": []
    });
    std::fs::write(dir.join("accounts.json"), serde_json::to_vec(&raw).unwrap()).unwrap();
}

#[tokio::test]
async fn startup_migrates_legacy_plaintext_and_scrubs_the_file() {
    let (base, seen) = serve(handler(ok_chain())).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    legacy_file(tmp.path());
    let keys = Arc::new(MemoryDeviceKey::new());
    let (svc, _, _) = open(tmp.path(), &ep, keys.clone(), LoginOutcome::Cancelled);
    let text = std::fs::read_to_string(tmp.path().join("accounts.json")).unwrap();
    assert!(!text.contains("legacy-access-SECRET") && !text.contains("rt-legacy"));
    assert!(!text.contains("accessToken") && !text.contains("msaRefreshToken"));
    assert!(text.contains("mc2:") && text.contains("keepMe"));
    let st = svc.app_state().unwrap();
    assert_eq!(st.accounts.len(), 2);
    assert!(st.accounts[0].credentials_persisted);
    // The migrated refresh token still works.
    svc.launch_credentials(UUID).await.unwrap();
    let body = String::from_utf8_lossy(&seen.lock().unwrap()[0].body).to_string();
    assert!(body.contains("refresh_token=rt-legacy"));
}

#[tokio::test]
async fn scrubbing_migration_leaves_no_plaintext_in_the_bak_copy() {
    let (base, _) = serve(handler(ok_chain())).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    legacy_file(tmp.path());
    // A previous generation from an older save still holds the plaintext.
    std::fs::copy(
        tmp.path().join("accounts.json"),
        tmp.path().join("accounts.json.bak"),
    )
    .unwrap();
    let keys = Arc::new(MemoryDeviceKey::new());
    let (_svc, _, _) = open(tmp.path(), &ep, keys, LoginOutcome::Cancelled);
    for name in ["accounts.json", "accounts.json.bak"] {
        let text = std::fs::read_to_string(tmp.path().join(name)).unwrap();
        assert!(
            !text.contains("legacy-access-SECRET") && !text.contains("rt-legacy"),
            "{name} still has plaintext tokens"
        );
    }
}

#[tokio::test]
async fn legacy_plaintext_is_scrubbed_even_when_the_keystore_is_locked() {
    let (base, _) = serve(handler(ok_chain())).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    legacy_file(tmp.path());
    let keys = Arc::new(MemoryDeviceKey::new());
    keys.set_locked(true);
    let (svc, _, _) = open(tmp.path(), &ep, keys, LoginOutcome::Cancelled);
    let text = std::fs::read_to_string(tmp.path().join("accounts.json")).unwrap();
    assert!(!text.contains("legacy-access-SECRET") && !text.contains("rt-legacy"));
    assert!(!text.contains("mc2:"));
    // Memory-only: usable now, but flagged as not persisted.
    let a = &svc.app_state().unwrap().accounts[0];
    assert!(!a.credentials_persisted && !a.needs_relogin);
}

#[tokio::test]
async fn accounts_json_from_another_device_cannot_be_loaded_as_persisted() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let original = f.account_json(UUID)["encryptedCredentials"].clone();
    let calls_before = f.seen.lock().unwrap().len();

    // The same file on a machine with a different device key.
    let other = Arc::new(MemoryDeviceKey::new());
    let (svc, _) = f.reopen(other);
    let a = svc.app_state().unwrap().accounts.remove(0);
    assert!(a.credentials_locked);
    assert!(a.needs_relogin);
    assert!(!a.credentials_persisted);
    let e = svc.launch_credentials(UUID).await.unwrap_err();
    assert!(e.needs_relogin);
    assert_eq!(f.seen.lock().unwrap().len(), calls_before, "no network use");
    // The foreign envelope is left exactly as it was.
    assert_eq!(f.account_json(UUID)["encryptedCredentials"], original);
}

#[tokio::test]
async fn locked_keystore_keeps_the_envelope_and_signs_in_memory_only() {
    let f = fx(ok_chain(), |ep| redirect(ep, "code=c")).await;
    f.svc.account_sign_in_microsoft().await.unwrap().unwrap();
    let original = f.account_json(UUID)["encryptedCredentials"].clone();

    f.keys.set_locked(true);
    let (svc, _, login) = open(
        f.tmp.path(),
        &f.ep,
        f.keys.clone(),
        redirect(&f.ep, "code=c2"),
    );
    let a = svc.app_state().unwrap().accounts.remove(0);
    assert!(a.credentials_locked && a.needs_relogin);

    // Signing in again works for this session but cannot be saved.
    let a = svc.account_sign_in_microsoft().await.unwrap().unwrap();
    assert_eq!(login.requests().len(), 1);
    assert!(!a.credentials_persisted);
    assert!(!a.needs_relogin && !a.credentials_locked);
    assert_eq!(f.account_json(UUID)["encryptedCredentials"], original);
    assert_no_secrets(&f.file());
    // Usable in memory right away.
    assert_eq!(
        svc.launch_credentials(UUID).await.unwrap().access_token(),
        "mc-token"
    );

    // Unlocked later: the next sign-in persists again.
    f.keys.set_locked(false);
    let a = svc.account_sign_in_microsoft().await.unwrap().unwrap();
    assert!(a.credentials_persisted);
    assert_ne!(f.account_json(UUID)["encryptedCredentials"], original);
    assert!(f.keys.get_or_create().is_ok());
}

#[tokio::test]
async fn electron_credential_blobs_need_one_relogin_and_are_kept() {
    let (base, seen) = serve(handler(ok_chain())).await;
    let ep = endpoints(&base);
    let tmp = tempfile::tempdir().unwrap();
    let raw = json!({"accounts": [{"type": "microsoft", "uuid": UUID, "name": "Steve",
        "encryptedCredentials": "djEwAAAA-electron-safestorage", "skins": [], "capes": []}],
        "profiles": []});
    std::fs::write(tmp.path().join("accounts.json"), raw.to_string()).unwrap();
    let (svc, _, _) = open(
        tmp.path(),
        &ep,
        Arc::new(MemoryDeviceKey::new()),
        LoginOutcome::Cancelled,
    );
    let a = svc.app_state().unwrap().accounts.remove(0);
    assert!(a.needs_relogin && !a.credentials_persisted);
    let e = svc.launch_credentials(UUID).await.unwrap_err();
    assert!(e.needs_relogin);
    assert!(seen.lock().unwrap().is_empty());
    let text = std::fs::read_to_string(tmp.path().join("accounts.json")).unwrap();
    assert!(text.contains("djEwAAAA-electron-safestorage"));
}

// ---------- events and contract shapes ----------

#[test]
fn accounts_changed_event_has_the_contract_name_and_payload() {
    let e = Event::AccountsChanged(AccountsChangedEvent { uuid: "u".into() });
    assert_eq!(e.name(), "accounts:changed");
    assert_eq!(e.payload(), json!({"uuid": "u"}));
}

#[test]
fn login_outcome_debug_hides_the_code() {
    let s = format!("{:?}", LoginOutcome::Code("https://x/?code=SECRET".into()));
    assert!(!s.contains("SECRET"));
}

#[tokio::test]
async fn only_one_sign_in_window_at_a_time() {
    use std::sync::atomic::AtomicBool;
    struct Slow(AtomicBool);
    #[async_trait::async_trait]
    impl LoginWindow for Slow {
        async fn authorize(&self, _: &str, _: &str) -> LoginOutcome {
            self.0.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(300)).await;
            LoginOutcome::Cancelled
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let slow = Arc::new(Slow(AtomicBool::new(false)));
    let svc = AppService::with_host(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions::default(),
        Host::system(false).with_login(slow.clone()),
    )
    .unwrap();
    let first = {
        let svc = svc.clone();
        tokio::spawn(async move { svc.account_sign_in_microsoft().await })
    };
    while !slow.0.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        code(svc.account_sign_in_microsoft().await.unwrap_err()),
        "auth.login_in_progress"
    );
    assert_eq!(first.await.unwrap().unwrap(), None);
    assert_eq!(svc.account_sign_in_microsoft().await.unwrap(), None);
}

#[test]
fn open_external_accepts_only_allowed_https_hosts() {
    #[derive(Default)]
    struct Rec(Mutex<Vec<String>>);
    impl ExternalOpener for Rec {
        fn open_url(&self, url: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let rec = Arc::new(Rec::default());
    let svc = AppService::with_host(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions::default(),
        Host::system(false).with_external(rec.clone()),
    )
    .unwrap();
    let ok = [
        "https://microsoft.com/",
        "https://support.xbox.com/help?x=1",
        "https://www.minecraft.net/en-us/login",
        "https://account.live.com/",
        "https://ACCOUNT.Microsoft.com/a",
    ];
    for u in ok {
        svc.open_external(u.into()).unwrap();
    }
    assert_eq!(rec.0.lock().unwrap().len(), ok.len());
    for bad in [
        "http://microsoft.com/",
        "https://evilmicrosoft.com/",
        "https://microsoft.com.evil.example/",
        "https://example.com/?u=microsoft.com",
        "https://user:pw@microsoft.com/",
        "https://user@xbox.com/",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "mailto:a@microsoft.com",
        "not a url",
        "",
    ] {
        let e = svc.open_external(bad.into()).unwrap_err();
        assert_eq!(e.code, "external_url_forbidden", "{bad}");
    }
    assert_eq!(rec.0.lock().unwrap().len(), ok.len());
}

#[test]
fn login_window_navigation_allow_list() {
    for ok in [
        "https://login.live.com/oauth20_authorize.srf?client_id=x",
        "https://account.live.com/password/reset",
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
        "https://www.microsoft.com/privacy",
        "https://xsts.auth.xboxlive.com/x",
        "https://aadcdn.msauth.net/shared/1.0/a.js",
        "https://logincdn.msftauth.net/x.css",
    ] {
        assert!(login_navigation_allowed(ok), "{ok}");
    }
    for bad in [
        "http://login.live.com/",
        "https://login.live.com.evil.example/",
        "https://evillive.com/",
        "https://example.com/?u=login.live.com",
        "https://u:p@login.live.com/",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "about:blank",
        "",
    ] {
        assert!(!login_navigation_allowed(bad), "{bad}");
    }
    let r = "https://login.live.com/oauth20_desktop.srf";
    assert!(is_login_redirect(&format!("{r}?code=1"), r));
    assert!(!is_login_redirect("https://login.live.com/other", r));
}

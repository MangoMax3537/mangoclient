#![allow(clippy::unwrap_used)]

use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

// ---- vault -------------------------------------------------------------

#[derive(Default)]
struct MockLegacy {
    data: Mutex<HashMap<String, String>>,
}

impl CredentialStore for MockLegacy {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }
    fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}

const RT: &str = "M.C5_BAY.0.U.-REFRESH-SECRET-xyz";
const AT: &str = "eyJ-ACCESS-SECRET-abc";

fn uuid() -> String {
    "0123456789abcdef0123456789abcdef".into()
}

fn vault_with(keys: Arc<MemoryDeviceKey>, legacy: Arc<MockLegacy>) -> Vault {
    Vault::new(keys, Some(legacy))
}

fn ms_account(rt: Option<&str>) -> Account {
    let mut a = offline_account("Player").unwrap();
    a.kind = AccountType::Microsoft;
    a.uuid = uuid();
    a.access_token = AT.into();
    a.msa_refresh_token = rt.map(str::to_string);
    a.expires_at = Some(crate::account::now_ms() + 3_600_000);
    a
}

fn env_of(m: &Map<String, Value>) -> String {
    m["encryptedCredentials"].as_str().unwrap().to_string()
}

#[test]
fn round_trip_persists_only_refresh_token() {
    let keys = Arc::new(MemoryDeviceKey::new());
    let v = vault_with(keys, Arc::default());
    let st = v.store_account(&ms_account(Some(RT))).unwrap();
    assert!(st.persisted);
    let env = env_of(&st.value);
    assert!(env.starts_with("mc2:"));
    let text = serde_json::to_string(&st.value).unwrap();
    assert!(!text.contains("REFRESH") && !text.contains("ACCESS"));
    assert!(!st.value.contains_key("accessToken"));
    assert!(!st.value.contains_key("msaRefreshToken"));
    let back = v.load_account(&st.value).unwrap();
    assert_eq!(back.msa_refresh_token.as_deref(), Some(RT));
    assert!(back.access_token.is_empty());
    assert!(!back.credentials_locked && !back.memory_only);
    // Random nonce: two seals differ.
    assert_ne!(env, env_of(&v.store_account(&back).unwrap().value));
}

fn flip(env: &str, byte: usize) -> String {
    let mut blob = STANDARD.decode(&env["mc2:".len()..]).unwrap();
    blob[byte] ^= 1;
    format!("mc2:{}", STANDARD.encode(blob))
}

#[test]
fn tampering_nonce_or_ciphertext_is_detected() {
    let key = DeviceKey::generate().unwrap();
    let env = seal_envelope(&key, &uuid(), RT).unwrap();
    assert_eq!(&*open_envelope(&key, &uuid(), &env).unwrap(), RT);
    assert!(open_envelope(&key, &uuid(), &flip(&env, 0)).is_err()); // nonce
    assert!(open_envelope(&key, &uuid(), &flip(&env, 30)).is_err()); // ciphertext
    let last = STANDARD.decode(&env[4..]).unwrap().len() - 1;
    assert!(open_envelope(&key, &uuid(), &flip(&env, last)).is_err()); // tag
    assert!(open_envelope(&key, &uuid(), "mc2:AAAA").is_err());
    assert!(open_envelope(&key, &uuid(), "mc2:!!!").is_err());
}

#[test]
fn envelope_is_bound_to_account_and_device() {
    let a = DeviceKey::generate().unwrap();
    let b = DeviceKey::generate().unwrap();
    let env = seal_envelope(&a, &uuid(), RT).unwrap();
    assert!(open_envelope(&a, &"f".repeat(32), &env).is_err());
    assert!(open_envelope(&b, &uuid(), &env).is_err());
    // Loaded on another device the account needs attention but still loads.
    let va = Vault::new(Arc::new(MemoryDeviceKey::with_key(a)), None);
    let vb = Vault::new(Arc::new(MemoryDeviceKey::with_key(b)), None);
    let st = va.store_account(&ms_account(Some(RT))).unwrap();
    let other = vb.load_account(&st.value).unwrap();
    assert!(other.msa_refresh_token.is_none());
    assert!(other.credentials_locked);
    // ...and its envelope is kept untouched.
    assert_eq!(
        env_of(&vb.store_account(&other).unwrap().value),
        env_of(&st.value)
    );
}

#[test]
fn device_key_never_appears_in_accounts_json() {
    let key = DeviceKey::generate().unwrap();
    let key_b64 = STANDARD.encode(key.as_bytes());
    let key_hex: String = key.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let v = Vault::new(Arc::new(MemoryDeviceKey::with_key(key.clone())), None);
    let st = v.store_account(&ms_account(Some(RT))).unwrap();
    let text = serde_json::to_string(&st.value).unwrap();
    assert!(!text.contains(&key_b64) && !text.contains(&key_hex));
    assert!(!format!("{key:?}").contains(&key_b64));
}

#[test]
fn locked_store_preserves_envelope_and_keeps_new_tokens_in_memory() {
    let keys = Arc::new(MemoryDeviceKey::new());
    let v = vault_with(keys.clone(), Arc::default());
    let stored = v.store_account(&ms_account(Some(RT))).unwrap().value;
    keys.set_locked(true);
    let loaded = v.load_account(&stored).unwrap();
    assert!(loaded.credentials_locked);
    assert!(loaded.msa_refresh_token.is_none());
    let again = v.store_account(&loaded).unwrap();
    assert!(again.persisted);
    assert_eq!(again.value, stored); // byte-for-byte, lossless

    // A token obtained meanwhile (carrying the old envelope) is memory-only.
    let mut fresh = ms_account(Some("NEW-REFRESH"));
    fresh.encrypted_credentials = Some(env_of(&stored));
    let st = v.store_account(&fresh).unwrap();
    assert!(!st.persisted);
    assert_eq!(env_of(&st.value), env_of(&stored));
    assert!(!serde_json::to_string(&st.value).unwrap().contains("NEW-"));
    // Brand new login with the keystore down: no envelope at all.
    let st = v.store_account(&ms_account(Some("NEW-REFRESH"))).unwrap();
    assert!(!st.persisted);
    assert!(!st.value.contains_key("encryptedCredentials"));
    assert!(!serde_json::to_string(&st.value).unwrap().contains("NEW-"));
}

#[test]
fn migrates_legacy_plaintext_and_scrubs_it() {
    let keys = Arc::new(MemoryDeviceKey::new());
    let v = vault_with(keys, Arc::default());
    let legacy = obj(json!({
        "type": "microsoft", "uuid": uuid(), "name": "Player",
        "accessToken": AT, "msaRefreshToken": RT, "expiresAt": 5, "custom": 1
    }));
    assert!(has_plaintext_credentials(&legacy) && needs_migration(&legacy));
    let acc = v.load_account(&legacy).unwrap();
    assert!(acc.access_token.is_empty());
    assert_eq!(acc.msa_refresh_token.as_deref(), Some(RT));
    let st = v.store_account(&acc).unwrap();
    assert!(st.persisted);
    let text = serde_json::to_string(&st.value).unwrap();
    assert!(!text.contains("SECRET") && !text.contains("accessToken"));
    assert!(!has_plaintext_credentials(&st.value) && !needs_migration(&st.value));
    assert_eq!(st.value["custom"], json!(1));
    assert_eq!(
        v.load_account(&st.value)
            .unwrap()
            .msa_refresh_token
            .as_deref(),
        Some(RT)
    );
}

#[test]
fn plaintext_is_scrubbed_even_when_keystore_is_down() {
    let keys = Arc::new(MemoryDeviceKey::new());
    keys.set_locked(true);
    let v = vault_with(keys, Arc::default());
    let legacy = obj(json!({
        "type": "microsoft", "uuid": uuid(), "name": "P", "msaRefreshToken": RT
    }));
    let st = v.store_account(&v.load_account(&legacy).unwrap()).unwrap();
    assert!(!st.persisted);
    assert!(!serde_json::to_string(&st.value).unwrap().contains("SECRET"));
}

#[test]
fn migrates_v1_envelope_and_deletes_old_entry() {
    let keys = Arc::new(MemoryDeviceKey::new());
    let legacy = Arc::new(MockLegacy::default());
    legacy.data.lock().unwrap().insert(
        uuid(),
        json!({"accessToken": AT, "msaRefreshToken": RT}).to_string(),
    );
    let v = vault_with(keys, legacy.clone());
    let stored = obj(json!({
        "type": "microsoft", "uuid": uuid(), "name": "P",
        "encryptedCredentials": format!("{ENVELOPE_PREFIX}{}", uuid())
    }));
    assert!(needs_migration(&stored));
    let acc = v.load_account(&stored).unwrap();
    assert_eq!(acc.msa_refresh_token.as_deref(), Some(RT));
    assert!(acc.access_token.is_empty());
    assert_eq!(legacy.data.lock().unwrap().len(), 1); // not yet deleted
    let st = v.store_account(&acc).unwrap();
    assert!(env_of(&st.value).starts_with("mc2:"));
    assert!(legacy.data.lock().unwrap().is_empty());
    assert!(!needs_migration(&st.value));
}

#[test]
fn v1_envelope_survives_when_v2_cannot_be_written() {
    let keys = Arc::new(MemoryDeviceKey::new());
    let legacy = Arc::new(MockLegacy::default());
    legacy
        .data
        .lock()
        .unwrap()
        .insert(uuid(), json!({"msaRefreshToken": RT}).to_string());
    let v = vault_with(keys.clone(), legacy.clone());
    let stored = obj(json!({
        "type": "microsoft", "uuid": uuid(), "name": "P",
        "encryptedCredentials": format!("{ENVELOPE_PREFIX}{}", uuid())
    }));
    let acc = v.load_account(&stored).unwrap();
    keys.set_locked(true);
    let st = v.store_account(&acc).unwrap();
    assert!(!st.persisted);
    assert_eq!(
        st.value["encryptedCredentials"],
        stored["encryptedCredentials"]
    );
    assert_eq!(legacy.data.lock().unwrap().len(), 1);
}

#[test]
fn electron_envelope_is_a_distinct_error() {
    let v = Vault::new(Arc::new(MemoryDeviceKey::new()), None);
    let stored = obj(json!({
        "uuid": "e".repeat(32), "name": "P",
        "encryptedCredentials": STANDARD.encode(b"v11opaque")
    }));
    let err = v.load_account(&stored).unwrap_err();
    assert!(matches!(err, AuthError::LegacyEnvelope));
    assert!(err.needs_relogin());
    assert_eq!(err.code(), "auth.legacy_envelope");
}

#[test]
fn delete_credentials_removes_v1_entry() {
    let legacy = Arc::new(MockLegacy::default());
    legacy.data.lock().unwrap().insert(uuid(), "{}".into());
    let v = vault_with(Arc::new(MemoryDeviceKey::new()), legacy.clone());
    let stored = obj(json!({
        "uuid": uuid(), "encryptedCredentials": format!("{ENVELOPE_PREFIX}{}", uuid())
    }));
    v.delete_credentials(&stored).unwrap();
    assert!(legacy.data.lock().unwrap().is_empty());
}

#[test]
fn account_never_serializes_tokens_and_secrets_stay_out_of_debug_and_errors() {
    let a = ms_account(Some(RT));
    let json = serde_json::to_string(&a).unwrap();
    assert!(!json.contains("SECRET") && !json.contains("accessToken"));
    assert!(!json.contains("msaRefreshToken"));
    for s in [format!("{a:?}"), format!("{a:#?}")] {
        assert!(!s.contains("SECRET"));
    }
    let tokens = MsaTokens {
        access_token: AT.into(),
        refresh_token: Some(RT.into()),
    };
    assert!(!format!("{tokens:?}").contains("SECRET"));
    let key = DeviceKey::generate().unwrap();
    let env = seal_envelope(&key, &uuid(), RT).unwrap();
    let e = open_envelope(&key, "x", &env).unwrap_err();
    assert!(!format!("{e} {e:?}").contains("SECRET"));
    // Bad typed field must not echo the secret back.
    let bad = obj(json!({"uuid": uuid(), "expiresAt": "TOKEN-SECRET-VALUE"}));
    let v = Vault::new(Arc::new(MemoryDeviceKey::new()), None);
    let e = v.load_account(&bad).unwrap_err();
    assert!(!format!("{e} {e:?}").contains("SECRET"));
}

#[test]
fn envelope_file_has_no_token_substrings() {
    let v = Vault::new(Arc::new(MemoryDeviceKey::new()), None);
    let st = v.store_account(&ms_account(Some(RT))).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accounts.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&st.value).unwrap()).unwrap();
    let raw = std::fs::read(&path).unwrap();
    for needle in [RT, AT, "REFRESH", "ACCESS", "M.C5"] {
        assert!(!raw.windows(needle.len()).any(|w| w == needle.as_bytes()));
    }
}

// ---- accounts ----------------------------------------------------------

#[test]
fn offline_account_matches_vanilla_uuid() {
    let a = offline_account(" Notch ").unwrap();
    assert_eq!(a.uuid, "b50ad385829d3141a2167e7d7539ba7f");
    assert_eq!(a.access_token, "0");
    assert_eq!(a.expires_at, None);
    assert_eq!(a.kind, AccountType::Offline);
    for bad in ["ab", "waytoolongusername1", "bad name", "ümlaut", ""] {
        assert!(
            matches!(offline_account(bad), Err(AuthError::InvalidUsername)),
            "{bad}"
        );
    }
}

#[test]
fn debug_and_public_account_hide_tokens() {
    let mut a = offline_account("Player").unwrap();
    a.access_token = "SUPERSECRET".into();
    a.msa_refresh_token = Some("REFRESHSECRET".into());
    let dbg = format!("{a:?}");
    assert!(!dbg.contains("SECRET"));
    let tokens = MsaTokens {
        access_token: "SUPERSECRET".into(),
        refresh_token: Some("R".into()),
    };
    assert!(!format!("{tokens:?}").contains("SUPERSECRET"));
    let public = serde_json::to_string(&a.to_public()).unwrap();
    assert!(!public.contains("SECRET"));
    assert!(public.contains("hasCape"));
}

#[test]
fn public_account_expiry_and_cape() {
    let mut a = offline_account("Player").unwrap();
    a.kind = AccountType::Microsoft;
    a.expires_at = Some(1000);
    a.capes = vec![Texture {
        state: Some("ACTIVE".into()),
        ..Default::default()
    }];
    let p = a.to_public_at(2000);
    assert!(p.expired && p.has_cape);
    assert!(!a.to_public_at(500).expired);
}

#[test]
fn authorize_url_and_redirect_parsing() {
    let ep = Endpoints::default();
    let url = ep.build_authorize_url();
    assert!(url.starts_with("https://login.live.com/oauth20_authorize.srf?"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("prompt=select_account"));
    assert!(url.contains("MBI_SSL"));
    let r = "https://login.live.com/oauth20_desktop.srf";
    assert_eq!(ep.parse_redirect("https://login.live.com/other"), None);
    assert_eq!(ep.parse_redirect(r), None);
    assert_eq!(
        ep.parse_redirect(&format!("{r}?code=M.abc")),
        Some(Redirect::Code("M.abc".into()))
    );
    assert_eq!(
        ep.parse_redirect(&format!("{r}?error=access_denied")),
        Some(Redirect::Cancelled)
    );
    assert_eq!(
        ep.parse_redirect(&format!("{r}?error=server_error&error_description=Too+bad")),
        Some(Redirect::Error("Too bad".into()))
    );
}

// ---- mock server -------------------------------------------------------

struct Req {
    method: String,
    path: String,
    headers: String,
    body: Vec<u8>,
}
type Handler = Arc<dyn Fn(&Req) -> (u16, Vec<u8>) + Send + Sync>;

async fn serve(handler: Handler) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let head_end = loop {
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break p + 4;
                    }
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let len = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                    })
                    .unwrap_or(0);
                while buf.len() < head_end + len {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let mut parts = head.split_whitespace();
                let req = Req {
                    method: parts.next().unwrap_or("").into(),
                    path: parts.next().unwrap_or("/").into(),
                    headers: head.to_ascii_lowercase(),
                    body: buf[head_end..].to_vec(),
                };
                let (status, body) = handler(&req);
                let out = format!(
                    "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(out.as_bytes()).await;
                let _ = sock.write_all(&body).await;
            });
        }
    });
    base
}

fn jr(status: u16, v: Value) -> (u16, Vec<u8>) {
    (status, serde_json::to_vec(&v).unwrap())
}

fn test_endpoints(base: &str) -> Endpoints {
    Endpoints {
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
        ..Endpoints::default()
    }
}

struct Chain {
    xsts: (u16, Value),
    entitlements: (u16, Value),
    profile: (u16, Value),
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

fn chain_handler(c: Chain) -> Handler {
    let seen = c.seen.clone();
    Arc::new(move |r: &Req| {
        let body = String::from_utf8_lossy(&r.body).to_string();
        seen.lock().unwrap().push((r.path.clone(), body));
        match r.path.as_str() {
            "/live/token" => jr(
                200,
                json!({"access_token": "live-at", "refresh_token": "live-rt"}),
            ),
            "/xbl" => jr(200, json!({"Token": "xbl-token"})),
            "/xsts" => jr(c.xsts.0, c.xsts.1.clone()),
            "/mc/login" => jr(200, json!({"access_token": "mc-token", "expires_in": 3600})),
            "/mc/ent" => jr(c.entitlements.0, c.entitlements.1.clone()),
            "/mc/profile" => {
                assert!(r.headers.contains("authorization: bearer mc-token"));
                jr(c.profile.0, c.profile.1.clone())
            }
            _ => (404, vec![]),
        }
    })
}

fn ok_chain() -> Chain {
    Chain {
        xsts: (
            200,
            json!({"Token": "xsts-token", "DisplayClaims": {"xui": [{"uhs": "uhs123"}]}}),
        ),
        entitlements: (200, json!({"items": [{"name": "game_minecraft"}]})),
        profile: (
            200,
            json!({"id": "0123456789abcdef0123456789abcdef", "name": "Steve",
                   "skins": [{"id": "s", "state": "ACTIVE", "url": "http://x/skin", "variant": "SLIM"}],
                   "capes": [{"state": "ACTIVE", "url": "http://x/cape"}]}),
        ),
        seen: Arc::new(Mutex::new(vec![])),
    }
}

#[tokio::test]
async fn full_chain_succeeds_with_exact_payloads() {
    let c = ok_chain();
    let seen = c.seen.clone();
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    let tokens = exchange_code(&ep, "the-code").await.unwrap();
    assert_eq!(tokens.access_token, "live-at");
    let acc = login_with_tokens(&ep, &tokens).await.unwrap();
    assert_eq!(acc.kind, AccountType::Microsoft);
    assert_eq!(acc.uuid, "0123456789abcdef0123456789abcdef");
    assert_eq!(acc.name, "Steve");
    assert_eq!(acc.access_token, "mc-token");
    assert_eq!(acc.msa_refresh_token.as_deref(), Some("live-rt"));
    assert!(acc.to_public().has_cape);
    assert!(acc.expires_at.unwrap() > 3_000_000_000_000_i64 / 2);

    let seen = seen.lock().unwrap().clone();
    let body = |p: &str| -> Value {
        let b = &seen.iter().find(|(path, _)| path == p).unwrap().1;
        serde_json::from_str(b).unwrap_or(Value::String(b.clone()))
    };
    let form = body("/live/token");
    let form = form.as_str().unwrap();
    assert!(form.contains("grant_type=authorization_code") && form.contains("code=the-code"));
    let xbl = body("/xbl");
    assert_eq!(xbl["RelyingParty"], "http://auth.xboxlive.com");
    assert_eq!(xbl["Properties"]["RpsTicket"], "live-at");
    assert_eq!(xbl["Properties"]["AuthMethod"], "RPS");
    let xsts = body("/xsts");
    assert_eq!(xsts["RelyingParty"], "rp://api.minecraftservices.com/");
    assert_eq!(xsts["Properties"]["SandboxId"], "RETAIL");
    assert_eq!(xsts["Properties"]["UserTokens"][0], "xbl-token");
    assert_eq!(
        body("/mc/login")["identityToken"],
        "XBL3.0 x=uhs123;xsts-token"
    );
}

#[tokio::test]
async fn xerr_codes_map_to_errors() {
    for (x, code) in [
        (2148916233_u64, "auth.xbox.no_profile"),
        (2148916235, "auth.xbox.country"),
        (2148916236, "auth.xbox.adult_verification"),
        (2148916237, "auth.xbox.adult_verification"),
        (2148916238, "auth.xbox.child_account"),
    ] {
        let mut c = ok_chain();
        c.xsts = (401, json!({"XErr": x}));
        let ep = test_endpoints(&serve(chain_handler(c)).await);
        let err = minecraft_login(&ep, "t").await.unwrap_err();
        assert_eq!(err.code(), code);
    }
    let mut c = ok_chain();
    c.xsts = (401, json!({"XErr": 1}));
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    assert!(matches!(
        minecraft_login(&ep, "t").await,
        Err(AuthError::Http { status: 401, .. })
    ));
}

#[tokio::test]
async fn profile_404_distinguishes_ownership_and_ignores_entitlement_failure() {
    let mut c = ok_chain();
    c.profile = (404, json!({}));
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    assert!(matches!(
        minecraft_login(&ep, "t").await,
        Err(AuthError::NoProfile)
    ));

    let mut c = ok_chain();
    c.profile = (404, json!({}));
    c.entitlements = (200, json!({"items": []}));
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    assert!(matches!(
        minecraft_login(&ep, "t").await,
        Err(AuthError::NotOwned)
    ));

    // Entitlement endpoint failing counts as "owns nothing" but is not fatal.
    let mut c = ok_chain();
    c.entitlements = (403, json!({}));
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    assert!(minecraft_login(&ep, "t").await.is_ok());

    let mut c = ok_chain();
    c.profile = (403, json!({}));
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    assert!(matches!(
        minecraft_login(&ep, "t").await,
        Err(AuthError::ProfileHttp(403))
    ));
}

#[tokio::test]
async fn exchange_failure_reports_description() {
    let base = serve(Arc::new(|_| {
        jr(400, json!({"error_description": "bad code"}))
    }))
    .await;
    let ep = test_endpoints(&base);
    let err = exchange_code(&ep, "x").await.unwrap_err();
    assert!(matches!(&err, AuthError::LoginFailed(m) if m == "bad code"));
}

#[tokio::test]
async fn ensure_fresh_skips_refresh_when_valid_and_requires_relogin_without_token() {
    let ep = test_endpoints("http://127.0.0.1:1"); // must not be contacted
    let mut a = offline_account("Player").unwrap();
    assert!(ensure_fresh_account(&ep, a.clone()).await.is_ok());

    a.kind = AccountType::Microsoft;
    a.access_token = "tok".into();
    a.expires_at = Some(crate::account::now_ms() + 10 * 60 * 1000);
    let same = ensure_fresh_account(&ep, a.clone()).await.unwrap();
    assert_eq!(same.access_token, "tok");

    a.expires_at = Some(crate::account::now_ms() + 60 * 1000); // < 5 min left
    let err = ensure_fresh_account(&ep, a.clone()).await.unwrap_err();
    assert!(matches!(err, AuthError::NeedsRelogin));
    assert!(err.needs_relogin());
}

#[tokio::test]
async fn ensure_fresh_refreshes_expiring_account() {
    let c = ok_chain();
    let seen = c.seen.clone();
    let ep = test_endpoints(&serve(chain_handler(c)).await);
    let mut a = offline_account("Player").unwrap();
    a.kind = AccountType::Microsoft;
    a.access_token = "old".into();
    a.msa_refresh_token = Some("old-rt".into());
    a.expires_at = Some(crate::account::now_ms() + 1000);
    a.extra.insert("keep".into(), json!(1));
    let fresh = ensure_fresh_account(&ep, a).await.unwrap();
    assert_eq!(fresh.access_token, "mc-token");
    assert_eq!(fresh.msa_refresh_token.as_deref(), Some("live-rt"));
    assert_eq!(fresh.extra["keep"], json!(1));
    let first = seen.lock().unwrap()[0].clone();
    assert!(
        first.1.contains("grant_type=refresh_token") && first.1.contains("refresh_token=old-rt")
    );
}

#[tokio::test]
async fn refresh_rejection_needs_relogin_but_outage_does_not() {
    let base = serve(Arc::new(|_| jr(400, json!({"error": "invalid_grant"})))).await;
    let err = refresh(&test_endpoints(&base), "rt").await.unwrap_err();
    assert!(err.needs_relogin());
    let base = serve(Arc::new(|_| jr(503, json!({})))).await;
    let err = refresh(&test_endpoints(&base), "rt").await.unwrap_err();
    assert!(!err.needs_relogin());
}

#[tokio::test]
async fn post_retries_on_5xx() {
    let n = Arc::new(Mutex::new(0));
    let n2 = n.clone();
    let base = serve(Arc::new(move |_| {
        let mut n = n2.lock().unwrap();
        *n += 1;
        if *n < 3 {
            jr(503, json!({}))
        } else {
            jr(200, json!({"access_token": "a", "refresh_token": "r"}))
        }
    }))
    .await;
    let ep = Endpoints {
        retries: 4,
        ..test_endpoints(&base)
    };
    assert!(exchange_code(&ep, "c").await.is_ok());
    assert_eq!(*n.lock().unwrap(), 3);
}

// ---- skins -------------------------------------------------------------

fn textures_value(base: &str, slim: bool) -> String {
    let model = if slim {
        json!({"model": "slim"})
    } else {
        json!({})
    };
    let t = json!({"textures": {
        "SKIN": {"url": format!("{base}/tex/skin"), "metadata": model},
        "CAPE": {"url": format!("{base}/tex/cape")}
    }});
    STANDARD.encode(serde_json::to_vec(&t).unwrap())
}

#[tokio::test]
async fn skin_from_session_server_is_cached() {
    let base_cell = Arc::new(Mutex::new(String::new()));
    let bc = base_cell.clone();
    let paths = Arc::new(Mutex::new(Vec::<String>::new()));
    let p2 = paths.clone();
    let base = serve(Arc::new(move |r| {
        p2.lock().unwrap().push(r.path.clone());
        if r.path.starts_with("/session/") {
            let base = bc.lock().unwrap().clone();
            jr(
                200,
                json!({"properties": [{"name": "textures", "value": textures_value(&base, true)}]}),
            )
        } else if r.path == "/tex/skin" {
            (200, b"SKINPNG".to_vec())
        } else if r.path == "/tex/cape" {
            (200, b"CAPEPNG".to_vec())
        } else {
            (404, vec![])
        }
    }))
    .await;
    *base_cell.lock().unwrap() = base.clone();
    let ep = test_endpoints(&base);
    let cache = tempfile::tempdir().unwrap();
    let mut acc = offline_account("Player").unwrap();
    acc.kind = AccountType::Microsoft;
    acc.uuid = "01234567-89ab-cdef-0123-456789abcdef".into();
    let r = get_skin(&ep, cache.path(), &acc).await;
    assert_eq!(r.source, SkinSource::Session);
    assert!(r.slim);
    assert_eq!(
        r.skin,
        format!("data:image/png;base64,{}", STANDARD.encode(b"SKINPNG"))
    );
    assert_eq!(
        r.cape.as_deref(),
        Some(&*format!(
            "data:image/png;base64,{}",
            STANDARD.encode(b"CAPEPNG")
        ))
    );
    assert!(paths
        .lock()
        .unwrap()
        .iter()
        .any(|p| p == "/session/0123456789abcdef0123456789abcdef?unsigned=false"));
    let cached = get_cached_skin(cache.path(), &acc.uuid).await.unwrap();
    assert_eq!(cached, r);
    assert!(cache
        .path()
        .join("skins")
        .join(format!("{}.json", acc.uuid))
        .exists());
}

#[tokio::test]
async fn skin_prefers_account_data_and_falls_back_to_steve() {
    let base = serve(Arc::new(|r| {
        if r.path == "/tex/skin" {
            (200, b"ACCSKIN".to_vec())
        } else {
            (500, vec![])
        }
    }))
    .await;
    let ep = test_endpoints(&base);
    let cache = tempfile::tempdir().unwrap();
    let mut acc = offline_account("Player").unwrap();
    acc.kind = AccountType::Microsoft;
    acc.skins = vec![Texture {
        state: Some("ACTIVE".into()),
        url: Some(format!("{base}/tex/skin")),
        variant: Some("SLIM".into()),
        ..Default::default()
    }];
    let r = get_skin(&ep, cache.path(), &acc).await;
    assert_eq!((r.source, r.slim), (SkinSource::Account, true));

    // Offline: no network at all, Steve.
    let off = offline_account("Player").unwrap();
    let r = get_skin(&ep, cache.path(), &off).await;
    assert_eq!(r.source, SkinSource::Default);
    assert_eq!(
        r.skin,
        format!(
            "data:image/png;base64,{}",
            STANDARD.encode(default_skin_png())
        )
    );
    // Steve is a 64x64 PNG.
    let png = default_skin_png();
    assert_eq!(&png[1..4], b"PNG");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 64);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 64);
}

#[tokio::test]
async fn cache_ignores_unsafe_uuid() {
    let cache = tempfile::tempdir().unwrap();
    assert!(get_cached_skin(cache.path(), "../evil").await.is_none());
}

#[tokio::test]
async fn upload_skin_sends_multipart_and_rejects_offline() {
    let captured = Arc::new(Mutex::new(None::<(String, String, Vec<u8>)>));
    let c2 = captured.clone();
    let base = serve(Arc::new(move |r| {
        *c2.lock().unwrap() = Some((r.method.clone(), r.headers.clone(), r.body.clone()));
        jr(200, json!({"id": "x"}))
    }))
    .await;
    let ep = test_endpoints(&base);
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("my skin.png");
    std::fs::write(&png, b"PNGDATA").unwrap();
    let mut acc = offline_account("Player").unwrap();
    assert!(matches!(
        upload_skin(&ep, &acc, &png, SkinVariant::Classic).await,
        Err(AuthError::OfflineSkin)
    ));
    acc.kind = AccountType::Microsoft;
    acc.access_token = "mc-token".into();
    let res = upload_skin(&ep, &acc, &png, SkinVariant::Slim)
        .await
        .unwrap();
    assert_eq!(res["id"], "x");
    let (method, headers, body) = captured.lock().unwrap().clone().unwrap();
    assert_eq!(method, "POST");
    assert!(headers.contains("authorization: bearer mc-token"));
    assert!(headers.contains("multipart/form-data; boundary="));
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"variant\"\r\n\r\nslim"));
    assert!(text.contains("filename=\"myskin.png\""));
    assert!(text.contains("PNGDATA"));

    let failing = serve(Arc::new(|_| (400, vec![]))).await;
    let err = upload_skin(&test_endpoints(&failing), &acc, &png, SkinVariant::Classic)
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::SkinUpload(400)));
}

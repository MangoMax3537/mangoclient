use crate::account::{now_ms, Account, AccountType, Texture};
use crate::endpoints::{Endpoints, SCOPE, XBL_RELYING_PARTY, XSTS_RELYING_PARTY};
use crate::error::AuthError;
use crate::http::{post_form, post_json};
use mango_net::{fetch_with_retry, get_json};
use serde_json::{json, Map, Value};
use std::fmt;

/// Live (MSA) tokens. Redacted in `Debug`.
#[derive(Clone)]
pub struct MsaTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl fmt::Debug for MsaTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MsaTokens")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

fn tokens_from(json: &Value) -> Result<MsaTokens, AuthError> {
    let access = json
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AuthError::LoginFailed("no access token in response".into()))?;
    Ok(MsaTokens {
        access_token: access.to_string(),
        refresh_token: json
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn description(json: &Value, fallback: &str) -> String {
    json.get("error_description")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

/// Redeem the authorization code from the sign-in redirect.
pub async fn exchange_code(ep: &Endpoints, code: &str) -> Result<MsaTokens, AuthError> {
    let (status, json) = post_form(
        ep,
        &ep.live_token,
        &[
            ("client_id", &ep.client_id),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &ep.redirect_uri),
            ("scope", SCOPE),
        ],
    )
    .await?;
    if !(200..300).contains(&status) {
        return Err(AuthError::LoginFailed(description(
            &json,
            "Microsoft sign-in failed",
        )));
    }
    tokens_from(&json)
}

/// Exchange a refresh token. A rejection (4xx) means the user must sign in
/// again; an outage (5xx) is reported as a plain failure so the session survives.
pub async fn refresh(ep: &Endpoints, refresh_token: &str) -> Result<MsaTokens, AuthError> {
    let (status, json) = post_form(
        ep,
        &ep.live_token,
        &[
            ("client_id", &ep.client_id),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
            ("redirect_uri", &ep.redirect_uri),
            ("scope", SCOPE),
        ],
    )
    .await?;
    if (200..300).contains(&status) {
        return tokens_from(&json).map_err(|_| AuthError::NeedsRelogin);
    }
    if (400..500).contains(&status) {
        return Err(AuthError::NeedsRelogin);
    }
    Err(AuthError::LoginFailed(description(
        &json,
        "Microsoft token refresh failed",
    )))
}

fn xsts_error(err: AuthError) -> AuthError {
    if let AuthError::Http { xerr: Some(x), .. } = &err {
        match x.as_str() {
            "2148916233" => return AuthError::XboxNoProfile,
            "2148916235" => return AuthError::XboxCountry,
            "2148916236" | "2148916237" => return AuthError::XboxAdultVerification,
            "2148916238" => return AuthError::XboxChildAccount,
            _ => {}
        }
    }
    err
}

fn textures(profile: &Value, key: &str) -> Vec<Texture> {
    profile
        .get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Live access token -> XBL -> XSTS -> Minecraft token -> profile. The
/// returned account has no refresh token yet.
pub async fn minecraft_login(
    ep: &Endpoints,
    live_access_token: &str,
) -> Result<Account, AuthError> {
    let xbl = post_json(
        ep,
        &ep.xbl_auth,
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                // MBI_SSL tickets go in verbatim; only AAD tokens need "d=".
                "RpsTicket": live_access_token,
            },
            "RelyingParty": XBL_RELYING_PARTY,
            "TokenType": "JWT",
        }),
    )
    .await?;
    let xbl_token = xbl.get("Token").and_then(Value::as_str).unwrap_or_default();

    let xsts = post_json(
        ep,
        &ep.xsts_auth,
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": XSTS_RELYING_PARTY,
            "TokenType": "JWT",
        }),
    )
    .await
    .map_err(xsts_error)?;
    let uhs = xsts
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let xsts_token = xsts
        .get("Token")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mc = post_json(
        ep,
        &ep.mc_login,
        &json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }),
    )
    .await?;
    let mc_token = mc
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| AuthError::LoginFailed("no Minecraft token in response".into()))?
        .to_string();
    let expires_in = mc
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .unwrap_or(86400);

    let headers = vec![("Authorization".to_string(), format!("Bearer {mc_token}"))];
    // Entitlement lookup failures are ignored; it only refines the 404 message.
    let owns = get_json::<Value>(&ep.mc_entitlements, &ep.fetch_opts(headers.clone()))
        .await
        .ok()
        .and_then(|v| {
            v.get("items")
                .and_then(Value::as_array)
                .map(|a| !a.is_empty())
        })
        .unwrap_or(false);

    let res = fetch_with_retry(&ep.mc_profile, &ep.fetch_opts(headers)).await?;
    let status = res.status();
    if status.as_u16() == 404 {
        return Err(if owns {
            AuthError::NoProfile
        } else {
            AuthError::NotOwned
        });
    }
    if !status.is_success() {
        return Err(AuthError::ProfileHttp(status.as_u16()));
    }
    let profile: Value = res.json().await?;
    let text = |k: &str| {
        profile
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };

    Ok(Account {
        kind: AccountType::Microsoft,
        uuid: text("id"),
        name: text("name"),
        access_token: mc_token,
        msa_refresh_token: None,
        expires_at: Some(now_ms().saturating_add(expires_in.saturating_mul(1000))),
        skins: textures(&profile, "skins"),
        capes: textures(&profile, "capes"),
        credentials_locked: false,
        encrypted_credentials: None,
        extra: Map::new(),
    })
}

/// Full chain from Live tokens, keeping the refresh token on the account.
pub async fn login_with_tokens(ep: &Endpoints, tokens: &MsaTokens) -> Result<Account, AuthError> {
    let mut account = minecraft_login(ep, &tokens.access_token).await?;
    account.msa_refresh_token = tokens.refresh_token.clone();
    Ok(account)
}

/// Account with a fresh Minecraft token, refreshing only when under 5 minutes
/// remain. Offline accounts pass through.
pub async fn ensure_fresh_account(ep: &Endpoints, account: Account) -> Result<Account, AuthError> {
    if account.kind == AccountType::Offline {
        return Ok(account);
    }
    if !account.access_token.is_empty()
        && matches!(account.expires_at, Some(e) if e > 0 && e - now_ms() > 5 * 60 * 1000)
    {
        return Ok(account);
    }
    let Some(rt) = account.msa_refresh_token.clone().filter(|s| !s.is_empty()) else {
        return Err(AuthError::NeedsRelogin);
    };
    let tokens = refresh(ep, &rt).await?;
    // Microsoft may omit a rotated refresh token; keep the old one then.
    let tokens = MsaTokens {
        refresh_token: tokens.refresh_token.or(Some(rt)),
        ..tokens
    };
    let fresh = login_with_tokens(ep, &tokens).await?;
    Ok(Account {
        access_token: fresh.access_token,
        msa_refresh_token: fresh.msa_refresh_token,
        expires_at: fresh.expires_at,
        uuid: fresh.uuid,
        name: fresh.name,
        skins: fresh.skins,
        capes: fresh.capes,
        credentials_locked: false,
        ..account
    })
}

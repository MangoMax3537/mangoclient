use mango_net::FetchOptions;
use std::time::Duration;
use url::Url;

pub const DEFAULT_CLIENT_ID: &str = "00000000402b5328";
pub const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";
pub const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
pub const XBL_RELYING_PARTY: &str = "http://auth.xboxlive.com";
pub const XSTS_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

/// Every URL the auth code talks to, injectable for tests.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub client_id: String,
    pub redirect_uri: String,
    pub live_authorize: String,
    pub live_token: String,
    pub xbl_auth: String,
    pub xsts_auth: String,
    pub mc_login: String,
    pub mc_profile: String,
    pub mc_entitlements: String,
    pub mc_skins: String,
    /// `PUT` (body `{"capeId": ...}`) selects a cape, `DELETE` hides it.
    pub mc_capes_active: String,
    /// `GET <base>/<name>` -> `{id, name}` (404 = unknown). The current
    /// Mojang endpoint (the older `users/profiles/minecraft/<name>` was
    /// deprecated in its favour).
    pub profile_lookup: String,
    /// The older endpoint (`204` = unknown), tried when the current one
    /// fails with anything but 404.
    pub profile_lookup_legacy: String,
    /// Without trailing slash; the uuid is appended.
    pub session_profile: String,
    /// Retries after the first attempt (5xx, 429, network errors).
    pub retries: u32,
    pub backoff: Duration,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            client_id: std::env::var("MANGOCLIENT_MSA_CLIENT_ID")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string()),
            redirect_uri: REDIRECT_URI.into(),
            live_authorize: "https://login.live.com/oauth20_authorize.srf".into(),
            live_token: "https://login.live.com/oauth20_token.srf".into(),
            xbl_auth: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts_auth: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            mc_login: "https://api.minecraftservices.com/authentication/login_with_xbox".into(),
            mc_profile: "https://api.minecraftservices.com/minecraft/profile".into(),
            mc_entitlements: "https://api.minecraftservices.com/entitlements/mcstore".into(),
            mc_skins: "https://api.minecraftservices.com/minecraft/profile/skins".into(),
            mc_capes_active: "https://api.minecraftservices.com/minecraft/profile/capes/active"
                .into(),
            profile_lookup: "https://api.mojang.com/minecraft/profile/lookup/name".into(),
            profile_lookup_legacy: "https://api.mojang.com/users/profiles/minecraft".into(),
            session_profile: "https://sessionserver.mojang.com/session/minecraft/profile".into(),
            retries: 4,
            backoff: Duration::from_millis(400),
        }
    }
}

/// Outcome of a navigation of the sign-in window that reached the redirect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redirect {
    Code(String),
    /// `error=access_denied`: the user closed or denied the sign-in.
    Cancelled,
    Error(String),
}

/// A fresh random `state` for one sign-in attempt (128 bits, hex).
pub fn new_oauth_state() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        // No secure randomness: a state that can never match is safer than a
        // guessable one (the sign-in then fails closed).
        return String::new();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Endpoints {
    /// The page the sign-in window should open. `state` binds the response
    /// to this attempt; pass the same value to [`Endpoints::parse_redirect`].
    pub fn build_authorize_url(&self, state: &str) -> String {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("scope", SCOPE),
            ("prompt", "select_account"),
            ("state", state),
        ];
        match Url::parse_with_params(&self.live_authorize, params) {
            Ok(u) => u.to_string(),
            Err(_) => self.live_authorize.clone(),
        }
    }

    /// Inspect a navigation. `None` while the user is still on login pages.
    /// A code is only returned when the response carries `expected_state`
    /// (an empty expected state never matches).
    pub fn parse_redirect(&self, url: &str, expected_state: &str) -> Option<Redirect> {
        if !url.starts_with(&self.redirect_uri) {
            return None;
        }
        let parsed = Url::parse(url).ok()?;
        let mut code = None;
        let mut error = None;
        let mut desc = None;
        let mut state = None;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "state" => state = Some(v.into_owned()),
                "code" => code = Some(v.into_owned()),
                "error" => error = Some(v.into_owned()),
                "error_description" => desc = Some(v.into_owned()),
                _ => {}
            }
        }
        if let Some(err) = error.filter(|e| !e.is_empty()) {
            if err == "access_denied" {
                return Some(Redirect::Cancelled);
            }
            return Some(Redirect::Error(
                desc.filter(|d| !d.is_empty()).unwrap_or(err),
            ));
        }
        let code = code.filter(|c| !c.is_empty())?;
        if expected_state.is_empty() || state.as_deref() != Some(expected_state) {
            return Some(Redirect::Error(
                "The sign-in response did not belong to this sign-in".to_string(),
            ));
        }
        Some(Redirect::Code(code))
    }

    pub(crate) fn fetch_opts(&self, headers: Vec<(String, String)>) -> FetchOptions {
        FetchOptions {
            retries: self.retries,
            backoff: self.backoff,
            headers,
            ..FetchOptions::default()
        }
    }
}

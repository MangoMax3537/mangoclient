use crate::account::{Account, AccountType, Texture};
use crate::endpoints::Endpoints;
use crate::error::AuthError;
use crate::http::send_json;
use base64::{engine::general_purpose::STANDARD, Engine};
use mango_net::{get_bytes, get_json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

const STEVE_PNG: &[u8] = include_bytes!("../assets/steve.png");

/// Mojang's official 64x64 classic Steve.
pub fn default_skin_png() -> &'static [u8] {
    STEVE_PNG
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinSource {
    Account,
    Session,
    Default,
}

/// Same shape as the legacy cache file: PNGs as `data:image/png;base64,` URLs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinResult {
    pub skin: String,
    pub cape: Option<String>,
    pub slim: bool,
    pub source: SkinSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinVariant {
    Classic,
    Slim,
}

impl SkinVariant {
    fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Slim => "slim",
        }
    }
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

async fn fetch_data_url(ep: &Endpoints, url: &str) -> Result<String, AuthError> {
    Ok(data_url(&get_bytes(url, &ep.fetch_opts(vec![])).await?))
}

/// Only plain uuids may name a cache file.
fn cache_file(cache_root: &Path, uuid: &str) -> Option<PathBuf> {
    let ok = !uuid.is_empty()
        && uuid.len() <= 36
        && uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    ok.then(|| cache_root.join("skins").join(format!("{uuid}.json")))
}

/// (skin url, cape url, slim) from the base64 `textures` property.
fn decode_textures(value: &str) -> Option<(Option<String>, Option<String>, bool)> {
    let raw = STANDARD.decode(value).ok()?;
    let json: Value = serde_json::from_slice(&raw).ok()?;
    let tex = json.get("textures")?;
    let url = |k: &str| {
        tex.pointer(&format!("/{k}/url"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let slim = tex.pointer("/SKIN/metadata/model").and_then(Value::as_str) == Some("slim");
    Some((url("SKIN"), url("CAPE"), slim))
}

/// Resolve a player's skin and cape: account data first, then the public
/// session server, then Steve. Never fails; the result is cached best-effort
/// under `<cache_root>/skins/<uuid>.json`.
pub async fn get_skin(ep: &Endpoints, cache_root: &Path, account: &Account) -> SkinResult {
    let mut result = SkinResult {
        skin: String::new(),
        cape: None,
        slim: false,
        source: SkinSource::Default,
    };

    let from_account = async {
        let mut r = (None, None, false);
        if let Some(url) = account.skins.iter().find(|s| s.is_active()).and_then(|s| {
            s.url
                .as_deref()
                .map(|u| (u, s.variant.as_deref().unwrap_or("")))
        }) {
            r.0 = Some(fetch_data_url(ep, url.0).await?);
            r.2 = url.1.to_ascii_lowercase().contains("slim");
        }
        if let Some(url) = account
            .capes
            .iter()
            .find(|c| c.is_active())
            .and_then(|c| c.url.as_deref())
        {
            r.1 = Some(fetch_data_url(ep, url).await?);
        }
        Ok::<_, AuthError>(r)
    };
    if let Ok((skin, cape, slim)) = from_account.await {
        if let Some(s) = skin {
            result.skin = s;
            result.slim = slim;
            result.source = SkinSource::Account;
        }
        result.cape = cape;
    }

    if result.skin.is_empty() && account.kind != AccountType::Offline {
        let session = async {
            let clean = account.uuid.replace('-', "");
            let url = format!("{}/{clean}?unsigned=false", ep.session_profile);
            let prof: Value = get_json(&url, &ep.fetch_opts(vec![])).await?;
            let value = prof
                .get("properties")
                .and_then(Value::as_array)
                .and_then(|a| {
                    a.iter()
                        .find(|p| p.get("name").and_then(Value::as_str) == Some("textures"))
                })
                .and_then(|p| p.get("value").and_then(Value::as_str))
                .and_then(decode_textures);
            let Some((skin_url, cape_url, slim)) = value else {
                return Ok::<_, AuthError>(None);
            };
            let skin = match skin_url {
                Some(u) => Some(fetch_data_url(ep, &u).await?),
                None => None,
            };
            let cape = match cape_url {
                Some(u) if result.cape.is_none() => fetch_data_url(ep, &u).await.ok(),
                _ => None,
            };
            Ok(Some((skin, cape, slim)))
        };
        if let Ok(Some((Some(skin), cape, slim))) = session.await {
            result.skin = skin;
            result.slim = slim;
            result.source = SkinSource::Session;
            if result.cape.is_none() {
                result.cape = cape;
            }
        }
    }

    if result.skin.is_empty() {
        result.skin = data_url(STEVE_PNG);
        result.slim = false;
        result.source = SkinSource::Default;
    }

    // A failed lookup (offline, server down) must not replace a skin that was
    // cached earlier with the default one.
    let keep_existing =
        result.source == SkinSource::Default && cached_file_exists(cache_root, &account.uuid);
    if let Some(file) = cache_file(cache_root, &account.uuid).filter(|_| !keep_existing) {
        let write = async {
            if let Some(dir) = file.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            let data = serde_json::to_vec(&result)?;
            tokio::fs::write(&file, data).await?;
            Ok::<_, AuthError>(())
        };
        if let Err(e) = write.await {
            tracing::debug!("skin cache write failed: {e}");
        }
    }
    result
}

fn cached_file_exists(cache_root: &Path, uuid: &str) -> bool {
    cache_file(cache_root, uuid).is_some_and(|f| f.is_file())
}

pub async fn get_cached_skin(cache_root: &Path, uuid: &str) -> Option<SkinResult> {
    let file = cache_file(cache_root, uuid)?;
    let data = tokio::fs::read(file).await.ok()?;
    serde_json::from_slice(&data).ok()
}

/// The skin and cape lists of the signed-in account.
#[derive(Debug, Clone, Default)]
pub struct ProfileTextures {
    pub skins: Vec<Texture>,
    pub capes: Vec<Texture>,
}

/// Map a non-success status of an authenticated Minecraft services call.
fn services_error(status: u16) -> AuthError {
    match status {
        401 | 403 => AuthError::NeedsRelogin,
        404 => AuthError::NoProfile,
        s => AuthError::ProfileHttp(s),
    }
}

fn require_microsoft(account: &Account) -> Result<(), AuthError> {
    if account.kind == AccountType::Offline {
        return Err(AuthError::OfflineSkin);
    }
    if account.access_token.is_empty() {
        return Err(AuthError::NeedsRelogin);
    }
    Ok(())
}

/// `GET /minecraft/profile` with the account's access token: the current
/// skins and capes (with ids and aliases).
pub async fn fetch_profile_textures(
    ep: &Endpoints,
    account: &Account,
) -> Result<ProfileTextures, AuthError> {
    require_microsoft(account)?;
    let (status, json) = send_json(ep, |c| {
        c.get(&ep.mc_profile)
            .bearer_auth(&account.access_token)
            .header("Accept", "application/json")
    })
    .await?;
    if !(200..300).contains(&status) {
        return Err(services_error(status));
    }
    let list = |k: &str| -> Vec<Texture> {
        json.get(k)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    };
    Ok(ProfileTextures {
        skins: list("skins"),
        capes: list("capes"),
    })
}

/// Download one texture PNG (a skin or cape URL from the profile APIs).
pub async fn fetch_texture_bytes(ep: &Endpoints, url: &str) -> Result<Vec<u8>, AuthError> {
    Ok(get_bytes(url, &ep.fetch_opts(vec![])).await?)
}

/// A player's public skin (PNG bytes, slim model) from the session server.
/// `None` when the profile carries no skin texture.
pub async fn fetch_public_skin(
    ep: &Endpoints,
    uuid: &str,
) -> Result<Option<(Vec<u8>, bool)>, AuthError> {
    let clean = uuid.replace('-', "");
    if clean.is_empty() || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AuthError::InvalidAccount);
    }
    let url = format!("{}/{clean}?unsigned=false", ep.session_profile);
    let prof: Value = get_json(&url, &ep.fetch_opts(vec![])).await?;
    let textures = prof
        .get("properties")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|p| p.get("name").and_then(Value::as_str) == Some("textures"))
        })
        .and_then(|p| p.get("value").and_then(Value::as_str))
        .and_then(decode_textures);
    let Some((Some(skin_url), _, slim)) = textures else {
        return Ok(None);
    };
    Ok(Some((fetch_texture_bytes(ep, &skin_url).await?, slim)))
}

/// Resolve a Minecraft Java username to its uuid (dashless). `None` = no such
/// player. Tries the current Mojang endpoint, then the older one when the
/// current one is unavailable (anything but 404).
pub async fn lookup_player_uuid(ep: &Endpoints, name: &str) -> Result<Option<String>, AuthError> {
    let valid = (3..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err(AuthError::InvalidUsername);
    }
    let mut last = None;
    for base in [&ep.profile_lookup, &ep.profile_lookup_legacy] {
        let url = format!("{}/{name}", base.trim_end_matches('/'));
        let attempt = send_json(ep, |c| c.get(&url).header("Accept", "application/json")).await;
        match attempt {
            Ok((200, json)) => {
                let id = json
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|i| i.len() == 32 && i.bytes().all(|b| b.is_ascii_hexdigit()));
                return Ok(id.map(str::to_string));
            }
            // Unknown name on either endpoint.
            Ok((404 | 204, _)) => return Ok(None),
            Ok((status, _)) => last = Some(AuthError::ProfileHttp(status)),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or(AuthError::ProfileHttp(0)))
}

/// Upload a PNG file as the signed-in account's skin; returns the API's
/// profile JSON.
pub async fn upload_skin(
    ep: &Endpoints,
    account: &Account,
    png_path: &Path,
    variant: SkinVariant,
) -> Result<Value, AuthError> {
    if account.kind == AccountType::Offline {
        return Err(AuthError::OfflineSkin);
    }
    let data = tokio::fs::read(png_path).await?;
    let filename = png_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("skin.png");
    upload_skin_bytes(ep, account, &data, variant, filename).await
}

/// Upload PNG bytes as the signed-in account's skin (multipart: `variant`
/// and `file`). The bearer token only travels in the request header.
pub async fn upload_skin_bytes(
    ep: &Endpoints,
    account: &Account,
    data: &[u8],
    variant: SkinVariant,
    filename: &str,
) -> Result<Value, AuthError> {
    require_microsoft(account)?;
    let filename: String = filename
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    let filename = if filename.is_empty() {
        "skin.png".to_string()
    } else {
        filename
    };
    let boundary = format!(
        "----mangoclient{:016x}",
        data.len() as u64 ^ 0x6d61_6e67_6f00
    );
    let mut body = Vec::with_capacity(data.len() + 400);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
             Content-Type: image/png\r\n\r\n",
            variant.as_str()
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let ep_one = Endpoints {
        retries: 0,
        ..ep.clone()
    };
    let (status, json) = send_json(&ep_one, |c| {
        c.post(&ep.mc_skins)
            .bearer_auth(&account.access_token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body.clone())
    })
    .await?;
    if !(200..300).contains(&status) {
        return Err(if matches!(status, 401 | 403) {
            AuthError::NeedsRelogin
        } else {
            AuthError::SkinUpload(status)
        });
    }
    Ok(json)
}

/// Select the active cape (`Some(id)`) or hide it (`None`).
pub async fn set_active_cape(
    ep: &Endpoints,
    account: &Account,
    cape_id: Option<&str>,
) -> Result<Value, AuthError> {
    require_microsoft(account)?;
    let ep_one = Endpoints {
        retries: 0,
        ..ep.clone()
    };
    let (status, json) = send_json(&ep_one, |c| {
        let req = match cape_id {
            Some(id) => c
                .put(&ep.mc_capes_active)
                .json(&serde_json::json!({ "capeId": id })),
            None => c.delete(&ep.mc_capes_active),
        };
        req.bearer_auth(&account.access_token)
            .header("Accept", "application/json")
    })
    .await?;
    if !(200..300).contains(&status) {
        return Err(if matches!(status, 401 | 403) {
            AuthError::NeedsRelogin
        } else {
            AuthError::SkinUpload(status)
        });
    }
    Ok(json)
}

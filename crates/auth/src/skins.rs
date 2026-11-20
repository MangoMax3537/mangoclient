use crate::account::{Account, AccountType, Texture};
use crate::endpoints::Endpoints;
use crate::error::AuthError;
use crate::http::{send_json, send_raw_on};
use base64::{engine::general_purpose::STANDARD, Engine};
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
    /// Texture URL of `skin` / `cape` (textures are content-addressed: the
    /// same URL is the same image, so an unchanged URL is never downloaded
    /// again). Absent in files written by older versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cape_url: Option<String>,
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
    Ok(data_url(&fetch_texture_bytes(ep, url).await?))
}

/// The session-server profile of `uuid` (the public textures), as JSON.
async fn session_profile(ep: &Endpoints, uuid: &str) -> Result<Value, AuthError> {
    let clean = uuid.replace('-', "");
    let url = format!("{}/{clean}?unsigned=false", ep.session_profile);
    let (status, json) =
        send_json(ep, |c| c.get(&url).header("Accept", "application/json")).await?;
    if (200..300).contains(&status) && status != 204 {
        Ok(json)
    } else {
        Err(AuthError::ProfileHttp(status))
    }
}

/// `skin` (base64 data URL) of the cached result when it was made from `url`.
fn reuse<'a>(
    cached: Option<&'a SkinResult>,
    url: &str,
    pick: fn(&SkinResult) -> Option<&str>,
) -> Option<&'a SkinResult> {
    cached.filter(|c| pick(c) == Some(url))
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
/// under `<cache_root>/skins/<uuid>.json`. Textures whose URL equals the one
/// of the cached result are not downloaded again, and the skin and the cape
/// are fetched concurrently.
pub async fn get_skin(ep: &Endpoints, cache_root: &Path, account: &Account) -> SkinResult {
    let started = std::time::Instant::now();
    let previous = get_cached_skin(cache_root, &account.uuid).await;
    let mut result = SkinResult {
        skin: String::new(),
        cape: None,
        slim: false,
        source: SkinSource::Default,
        skin_url: None,
        cape_url: None,
    };

    // Skin and cape textures, concurrently; an unchanged URL comes from the cache.
    let load = |skin_url: Option<String>, cape_url: Option<String>| {
        let prev = previous.as_ref();
        async move {
            let skin = async {
                match skin_url {
                    Some(u) => match reuse(prev, &u, |c| c.skin_url.as_deref()) {
                        Some(c) => Ok(Some((c.skin.clone(), u))),
                        None => fetch_data_url(ep, &u).await.map(|d| Some((d, u))),
                    },
                    None => Ok(None),
                }
            };
            let cape = async {
                match cape_url {
                    Some(u) => match reuse(prev, &u, |c| c.cape_url.as_deref())
                        .and_then(|c| c.cape.clone())
                    {
                        Some(d) => Some((d, u)),
                        None => fetch_data_url(ep, &u).await.ok().map(|d| (d, u)),
                    },
                    None => None,
                }
            };
            tokio::join!(skin, cape)
        }
    };

    let acc_skin = account.skins.iter().find(|s| s.is_active()).and_then(|s| {
        s.url.clone().map(|u| {
            (
                u,
                s.variant
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("slim"),
            )
        })
    });
    let acc_cape = account
        .capes
        .iter()
        .find(|c| c.is_active())
        .and_then(|c| c.url.clone());
    if acc_skin.is_some() || acc_cape.is_some() {
        let slim = acc_skin.as_ref().is_some_and(|s| s.1);
        if let (Ok(skin), cape) = load(acc_skin.map(|s| s.0), acc_cape).await {
            if let Some((data, url)) = skin {
                result.skin = data;
                result.skin_url = Some(url);
                result.slim = slim;
                result.source = SkinSource::Account;
            }
            if let Some((data, url)) = cape {
                result.cape = Some(data);
                result.cape_url = Some(url);
            }
        }
    }

    if result.skin.is_empty() && account.kind != AccountType::Offline {
        let session = async {
            let prof = session_profile(ep, &account.uuid).await?;
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
            let cape_url = cape_url.filter(|_| result.cape.is_none());
            let (skin, cape) = load(skin_url, cape_url).await;
            Ok(Some((skin?, cape, slim)))
        };
        if let Ok(Some((Some((skin, skin_url)), cape, slim))) = session.await {
            result.skin = skin;
            result.skin_url = Some(skin_url);
            result.slim = slim;
            result.source = SkinSource::Session;
            if result.cape.is_none() {
                if let Some((data, url)) = cape {
                    result.cape = Some(data);
                    result.cape_url = Some(url);
                }
            }
        }
    }

    if result.skin.is_empty() {
        result.skin = data_url(STEVE_PNG);
        result.slim = false;
        result.source = SkinSource::Default;
        result.skin_url = None;
    }

    // A failed lookup (offline, server down) must not replace a skin that was
    // cached earlier with the default one; an identical result is not written
    // again (its file time versions the image URLs).
    let keep_existing = result.source == SkinSource::Default && previous.is_some();
    let unchanged = previous.as_ref() == Some(&result);
    if let Some(file) =
        cache_file(cache_root, &account.uuid).filter(|_| !keep_existing && !unchanged)
    {
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
    tracing::debug!(
        ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        source = ?result.source,
        "skin resolved"
    );
    result
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
    Ok(parse_textures(&json))
}

fn parse_textures(json: &Value) -> ProfileTextures {
    let list = |k: &str| -> Vec<Texture> {
        json.get(k)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    };
    ProfileTextures {
        skins: list("skins"),
        capes: list("capes"),
    }
}

/// The skin and cape lists of a profile response (`PUT .../capes/active`,
/// `POST .../skins` and `GET .../profile` all answer with the profile).
/// `None` when the answer carries no cape list (an empty or foreign body).
pub fn profile_textures(json: &Value) -> Option<ProfileTextures> {
    json.get("capes")?.as_array()?;
    Some(parse_textures(json))
}

/// Does `t` show the wanted cape as the one and only ACTIVE cape (`None`
/// wanted = no cape active)? Cape ids are the profile's `id`, not the alias.
pub fn cape_applied(t: &ProfileTextures, wanted: Option<&str>) -> bool {
    let active: Vec<&Texture> = t.capes.iter().filter(|c| c.is_active()).collect();
    match wanted {
        None => active.is_empty(),
        Some(id) => {
            active.len() == 1 && active[0].extra.get("id").and_then(Value::as_str) == Some(id)
        }
    }
}

/// Download one texture PNG (a skin or cape URL from the profile APIs).
pub async fn fetch_texture_bytes(ep: &Endpoints, url: &str) -> Result<Vec<u8>, AuthError> {
    /// A skin or cape PNG is far smaller; anything bigger is not one.
    const MAX_TEXTURE_BYTES: usize = 2 * 1024 * 1024;
    // The URL comes from profile data: only the texture host over https,
    // and every redirect hop must stay on it.
    if !ep.texture_policy.allows(url) {
        return Err(AuthError::TextureHost);
    }
    let client = mango_net::restricted_client(&ep.texture_policy)?;
    let (status, bytes) = match send_raw_on(client, ep, |c| c.get(url)).await {
        Err(AuthError::Request(e)) if e.is_redirect() => return Err(AuthError::TextureHost),
        other => other?,
    };
    if !(200..300).contains(&status) {
        return Err(AuthError::ProfileHttp(status));
    }
    if bytes.len() > MAX_TEXTURE_BYTES {
        return Err(AuthError::ProfileHttp(413));
    }
    Ok(bytes)
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
    let prof = session_profile(ep, &clean).await?;
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

    // Both calls replace the state (idempotent), so a bounded retry is safe.
    let ep_one = Endpoints {
        retries: ep.retries.min(2),
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
    // Both calls replace the state (idempotent), so a bounded retry is safe.
    let ep_one = Endpoints {
        retries: ep.retries.min(2),
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

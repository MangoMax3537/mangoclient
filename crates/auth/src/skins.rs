use crate::account::{Account, AccountType};
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

    if let Some(file) = cache_file(cache_root, &account.uuid) {
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

pub async fn get_cached_skin(cache_root: &Path, uuid: &str) -> Option<SkinResult> {
    let file = cache_file(cache_root, uuid)?;
    let data = tokio::fs::read(file).await.ok()?;
    serde_json::from_slice(&data).ok()
}

/// Upload a PNG as the signed-in account's skin; returns the API's profile JSON.
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
    let filename: String = png_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("skin.png")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
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
    body.extend_from_slice(&data);
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
        return Err(AuthError::SkinUpload(status));
    }
    Ok(json)
}

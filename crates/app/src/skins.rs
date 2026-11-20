//! Skins: the account's current skin and capes, changing them, copying
//! another player's skin and the local skin library.
//!
//! Tokens never leave Rust: every call that needs one takes the (refreshed)
//! runtime account, and no error, log line, event or response carries it.
//!
//! Cache layout (all under the data root):
//! - `cache/skins/<uuid>.json`: the skin of an account as `mango_auth::SkinResult`
//!   (also read by the `head/` and `skin/` image routes),
//! - `cache/skins/<uuid>-capes/<capeId>.png`: every cape of the account,
//! - `skins/index.json` and `skins/<id>.png`: the local library (user data,
//!   not cache; the index is written atomically).
//!
//! The default skin of an account without a texture is the bundled Steve; the
//! Java parity rule (`is_alex`) is implemented, but no Alex texture ships.

use crate::error::{AppError, Res};
use crate::events::{Event, SkinUpdatedEvent};
use crate::icon_images::write_atomic;
use crate::image_protocol::with_version;
use crate::path_grants::GrantKind;
use crate::service::{lock, now_ms, AppService};
use mango_auth::{
    Account as AuthAccount, AccountType as AuthKind, AuthError, SkinSource, SkinVariant,
};
use mango_core::Paths;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Largest skin or cape PNG that is uploaded, stored or downloaded.
pub const MAX_SKIN_BYTES: u64 = 1024 * 1024;
/// Skins in the local library.
pub const MAX_LIBRARY_SKINS: usize = 200;
const MAX_NAME_CHARS: usize = 48;
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SkinVariantName {
    Classic,
    Slim,
}

impl From<SkinVariantName> for SkinVariant {
    fn from(v: SkinVariantName) -> Self {
        match v {
            SkinVariantName::Classic => SkinVariant::Classic,
            SkinVariantName::Slim => SkinVariant::Slim,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SkinSourceName {
    Account,
    Session,
    Default,
}

impl From<SkinSource> for SkinSourceName {
    fn from(s: SkinSource) -> Self {
        match s {
            SkinSource::Account => SkinSourceName::Account,
            SkinSource::Session => SkinSourceName::Session,
            SkinSource::Default => SkinSourceName::Default,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CapeInfo {
    pub id: String,
    pub alias: String,
    /// `mangoimg://cape/<uuid>/<id>?v=<tag>`.
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkinInfo {
    /// `mangoimg://skin/<uuid>?v=<tag>`: the current skin (the bundled Steve
    /// when nothing is cached).
    pub skin_url: String,
    /// The active cape, else `null`.
    pub cape_url: Option<String>,
    pub slim: bool,
    pub source: SkinSourceName,
    pub capes: Vec<CapeInfo>,
    /// `false` for offline accounts (their skin cannot be changed).
    pub can_edit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySkin {
    pub id: String,
    pub name: String,
    pub variant: SkinVariantName,
    /// `mangoimg://libskin/<id>`.
    pub url: String,
    pub added_at: u64,
}

// ---- paths and ids ----

pub(crate) fn library_dir(paths: &Paths) -> PathBuf {
    paths.root().join("skins")
}

pub(crate) fn cape_dir(paths: &Paths, uuid: &str) -> PathBuf {
    paths.cache().join("skins").join(format!("{uuid}-capes"))
}

fn skin_cache_file(paths: &Paths, uuid: &str) -> PathBuf {
    paths.cache().join("skins").join(format!("{uuid}.json"))
}

/// A cape id as Mojang issues them (a uuid), kept loose but path-safe.
pub(crate) fn valid_cape_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub(crate) fn valid_library_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Tag that changes whenever `file` is rewritten.
fn file_tag(file: &Path) -> Option<String> {
    let meta = std::fs::metadata(file).ok()?;
    let nanos = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!("{nanos:x}{:x}", meta.len()))
}

/// Cache-busting tag of an account's cached skin (`None` = nothing cached).
pub(crate) fn skin_version(paths: &Paths, uuid: &str) -> Option<String> {
    if !crate::image_protocol::valid_head_id(uuid) {
        return None;
    }
    file_tag(&skin_cache_file(paths, uuid))
}

fn skin_digest(paths: &Paths, uuid: &str) -> Option<String> {
    let bytes = std::fs::read(skin_cache_file(paths, uuid)).ok()?;
    Some(hex::encode(Sha1::digest(bytes)))
}

// ---- validation ----

fn invalid_png(why: impl std::fmt::Display) -> AppError {
    AppError::new("skin.invalid_png", format!("Not a valid skin PNG: {why}"))
}

/// A real skin: a decodable PNG of 64x64 or 64x32 pixels, at most 1 MB.
pub(crate) fn validate_skin_png(bytes: &[u8]) -> Res<()> {
    if bytes.len() as u64 > MAX_SKIN_BYTES {
        return Err(AppError::new(
            "skin.too_large",
            "The skin file is larger than 1 MB",
        ));
    }
    if !bytes.starts_with(&PNG_MAGIC) {
        return Err(invalid_png("wrong file type"));
    }
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(invalid_png)?;
    let (w, h) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if w != 64 || !(h == 64 || h == 32) {
        return Err(invalid_png(format!("{w}x{h} (need 64x64 or 64x32)")));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| invalid_png("too large"))?;
    let mut buf = vec![0u8; size];
    reader.next_frame(&mut buf).map_err(invalid_png)?;
    Ok(())
}

/// A user-chosen skin file: a real file, at most 1 MB, a valid skin PNG.
fn read_skin_file(path: &Path) -> Res<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| AppError::new("skin.not_found", "The skin file was not found"))?;
    if !meta.is_file() {
        return Err(AppError::new(
            "skin.not_found",
            "The skin file was not found",
        ));
    }
    if meta.len() > MAX_SKIN_BYTES {
        return Err(AppError::new(
            "skin.too_large",
            "The skin file is larger than 1 MB",
        ));
    }
    let bytes = std::fs::read(path)?;
    validate_skin_png(&bytes)?;
    Ok(bytes)
}

fn clean_name(raw: Option<String>, fallback: &str) -> String {
    let name: String = raw
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let name = name.trim();
    let name = if name.is_empty() { fallback } else { name };
    name.chars().take(MAX_NAME_CHARS).collect()
}

/// Map the auth crate's skin errors onto stable codes.
fn skin_err(e: AuthError) -> AppError {
    match e {
        AuthError::OfflineSkin => AppError::new(
            "skin.offline_account",
            "Offline accounts cannot change their skin",
        ),
        AuthError::SkinUpload(status) => AppError::new(
            "skin.upload_failed",
            format!(
                "The skin service refused the change (HTTP {}xx)",
                status / 100
            ),
        ),
        other => other.into(),
    }
}

fn offline_refusal() -> AppError {
    skin_err(AuthError::OfflineSkin)
}

// ---- library files ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    id: String,
    name: String,
    variant: SkinVariantName,
    added_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IndexFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    skins: Vec<IndexEntry>,
}

fn library_url(id: &str) -> String {
    format!("mangoimg://libskin/{id}")
}

fn view(e: &IndexEntry) -> LibrarySkin {
    LibrarySkin {
        id: e.id.clone(),
        name: e.name.clone(),
        variant: e.variant,
        url: library_url(&e.id),
        added_at: e.added_at,
    }
}

/// Entries whose PNG still exists; a corrupt index is set aside and treated
/// as empty so the library keeps working.
fn load_index(paths: &Paths) -> Vec<IndexEntry> {
    let dir = library_dir(paths);
    let file = dir.join("index.json");
    let Ok(bytes) = std::fs::read(&file) else {
        return Vec::new();
    };
    let Ok(index) = serde_json::from_slice::<IndexFile>(&bytes) else {
        let _ = std::fs::rename(&file, dir.join("index.json.corrupt"));
        return Vec::new();
    };
    index
        .skins
        .into_iter()
        .filter(|e| valid_library_id(&e.id) && dir.join(format!("{}.png", e.id)).is_file())
        .collect()
}

fn save_index(paths: &Paths, skins: &[IndexEntry]) -> Res<()> {
    let text = serde_json::to_vec_pretty(&IndexFile {
        version: 1,
        skins: skins.to_vec(),
    })?;
    write_atomic(&library_dir(paths).join("index.json"), &text)?;
    Ok(())
}

fn library_add(
    paths: &Paths,
    name: Option<String>,
    variant: SkinVariantName,
    bytes: &[u8],
) -> Res<LibrarySkin> {
    let mut skins = load_index(paths);
    if skins.len() >= MAX_LIBRARY_SKINS {
        return Err(AppError::new(
            "skin.library_full",
            format!("The skin library holds at most {MAX_LIBRARY_SKINS} skins"),
        ));
    }
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.update(now_ms().to_le_bytes());
    hasher.update((skins.len() as u64).to_le_bytes());
    let id: String = hex::encode(hasher.finalize()).chars().take(32).collect();
    let entry = IndexEntry {
        id: id.clone(),
        name: clean_name(name, "Skin"),
        variant,
        added_at: now_ms(),
    };
    let png = library_dir(paths).join(format!("{id}.png"));
    write_atomic(&png, bytes)?;
    skins.push(entry.clone());
    if let Err(e) = save_index(paths, &skins) {
        let _ = std::fs::remove_file(&png);
        return Err(e);
    }
    Ok(view(&entry))
}

fn library_read(paths: &Paths, id: &str) -> Res<(IndexEntry, Vec<u8>)> {
    let not_found = || AppError::new("skin.not_found", "That skin is not in the library");
    if !valid_library_id(id) {
        return Err(not_found());
    }
    let entry = load_index(paths)
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(not_found)?;
    let bytes = std::fs::read(library_dir(paths).join(format!("{id}.png")))?;
    Ok((entry, bytes))
}

impl AppService {
    // ---- fetching and caching ----

    /// Fetch a Microsoft account's skin in the background (never blocks, the
    /// failure is only logged). `use_token`: refresh the token and read the
    /// profile first (sign-in, refresh); otherwise only the session server is
    /// asked (startup).
    pub(crate) fn spawn_skin_refresh(&self, uuid: String, use_token: bool) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let svc = self.clone();
        tokio::spawn(async move {
            if let Err(e) = svc.refresh_skin(&uuid, use_token, false).await {
                tracing::debug!("skin not refreshed: {}", e.code);
            }
        });
    }

    /// Once per app start: every Microsoft account, one after the other.
    pub(crate) fn spawn_skin_prefetch(&self) {
        let uuids: Vec<String> = self.inner.with_store(|s| {
            s.accounts
                .iter()
                .filter(|a| a.kind == mango_core::AccountType::Microsoft)
                .map(|a| a.uuid.clone())
                .collect()
        });
        if uuids.is_empty() {
            return;
        }
        let weak = std::sync::Arc::downgrade(&self.inner);
        let delay = self.inner.opts.skin_prefetch_delay;
        tokio::spawn(async move {
            for uuid in uuids {
                tokio::time::sleep(delay).await;
                let Some(inner) = weak.upgrade() else { return };
                let svc = AppService { inner };
                if let Err(e) = svc.refresh_skin(&uuid, false, false).await {
                    tracing::debug!("skin not prefetched: {}", e.code);
                }
            }
        });
    }

    /// Fetch and cache the skin and capes of `uuid`; emits `skin:updated` when
    /// the cached skin changed (or `force_emit`). Offline accounts: nothing.
    pub(crate) async fn refresh_skin(
        &self,
        uuid: &str,
        use_token: bool,
        force_emit: bool,
    ) -> Res<()> {
        let inner = &self.inner;
        let account = self.session_account(uuid)?;
        if account.kind == AuthKind::Offline {
            return Ok(());
        }
        let ep = inner.opts.auth_endpoints.clone();
        let cache_root = inner.paths.cache();
        let before = skin_digest(&inner.paths, uuid);

        let mut working: AuthAccount = account;
        let mut fresh = false;
        if use_token {
            match self.refresh_session(uuid, false).await {
                Ok(acc) => {
                    working = acc;
                    match mango_auth::fetch_profile_textures(&ep, &working).await {
                        Ok(t) => {
                            working.skins = t.skins.clone();
                            working.capes = t.capes.clone();
                            self.store_textures(uuid, t.skins, t.capes).await;
                            fresh = true;
                        }
                        Err(e) => tracing::debug!("profile textures not read: {}", e.code()),
                    }
                }
                Err(e) => tracing::debug!("token not refreshed for the skin: {}", e.code),
            }
        }
        if !fresh {
            // Possibly stale stored texture urls: ask the session server.
            working.skins.clear();
            working.capes.clear();
        }
        let _ = mango_auth::get_skin(&ep, &cache_root, &working).await;

        let capes = self
            .session_account(uuid)
            .map(|a| a.capes.clone())
            .unwrap_or_default();
        self.fetch_cape_files(uuid, &capes).await;

        let after = skin_digest(&inner.paths, uuid);
        if force_emit || before != after {
            inner.emit(Event::SkinUpdated(SkinUpdatedEvent {
                uuid: uuid.to_string(),
            }));
        }
        Ok(())
    }

    /// Download the capes that are not cached yet (best effort).
    async fn fetch_cape_files(&self, uuid: &str, capes: &[mango_auth::Texture]) {
        let ep = &self.inner.opts.auth_endpoints;
        let dir = cape_dir(&self.inner.paths, uuid);
        for cape in capes {
            let Some((id, url)) = cape_id(cape).zip(cape.url.as_deref()) else {
                continue;
            };
            let file = dir.join(format!("{id}.png"));
            if std::fs::metadata(&file).is_ok_and(|m| m.len() > 0) {
                continue;
            }
            match mango_auth::fetch_texture_bytes(ep, url).await {
                Ok(bytes)
                    if bytes.len() as u64 <= MAX_SKIN_BYTES && bytes.starts_with(&PNG_MAGIC) =>
                {
                    let _ = write_atomic(&file, &bytes);
                }
                Ok(_) => tracing::debug!("a cape texture was not a usable PNG"),
                Err(e) => tracing::debug!("cape not downloaded: {}", e.code()),
            }
        }
    }

    // ---- commands: current skin ----

    async fn build_skin_info(&self, uuid: &str) -> Res<SkinInfo> {
        let account = self.session_account(uuid)?;
        let paths = &self.inner.paths;
        let cached = mango_auth::get_cached_skin(&paths.cache(), uuid).await;
        let tag = skin_version(paths, uuid);
        let can_edit = account.kind == AuthKind::Microsoft;
        let mut capes = Vec::new();
        let mut active_url = None;
        for cape in &account.capes {
            let Some(id) = cape_id(cape) else { continue };
            let url = with_version(
                format!("mangoimg://cape/{uuid}/{id}"),
                file_tag(&cape_dir(paths, uuid).join(format!("{id}.png"))).as_deref(),
            );
            let active = cape.is_active();
            if active {
                active_url = Some(url.clone());
            }
            capes.push(CapeInfo {
                alias: cape
                    .extra
                    .get("alias")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id,
                url,
                active,
            });
        }
        Ok(SkinInfo {
            skin_url: with_version(format!("mangoimg://skin/{uuid}"), tag.as_deref()),
            cape_url: active_url,
            slim: cached.as_ref().is_some_and(|c| c.slim),
            source: cached.map_or(SkinSourceName::Default, |c| c.source.into()),
            capes,
            can_edit,
        })
    }

    /// The account's skin and capes. Microsoft accounts are (re)fetched first;
    /// when that fails the cached data is returned.
    pub async fn skin_get(&self, uuid: String) -> Res<SkinInfo> {
        let account = self.session_account(&uuid)?;
        if account.kind == AuthKind::Microsoft {
            if let Err(e) = self.refresh_skin(&uuid, true, false).await {
                tracing::debug!("skin_get: refresh failed: {}", e.code);
            }
        }
        self.build_skin_info(&uuid).await
    }

    fn microsoft_account(&self, uuid: &str) -> Res<AuthAccount> {
        let account = self.session_account(uuid)?;
        if account.kind == AuthKind::Offline {
            return Err(offline_refusal());
        }
        Ok(account)
    }

    /// Upload `bytes` as the account's skin, then refetch and announce it.
    async fn upload_and_refresh(
        &self,
        uuid: &str,
        bytes: &[u8],
        variant: SkinVariant,
        filename: &str,
    ) -> Res<SkinInfo> {
        let account = self.refresh_session(uuid, false).await?;
        let ep = self.inner.opts.auth_endpoints.clone();
        mango_auth::upload_skin_bytes(&ep, &account, bytes, variant, filename)
            .await
            .map_err(skin_err)?;
        self.refresh_skin(uuid, true, true).await?;
        self.build_skin_info(uuid).await
    }

    /// Upload a PNG chosen with `pick_image_file` as the account's skin.
    pub async fn skin_upload(
        &self,
        uuid: String,
        file_path: String,
        variant: SkinVariantName,
    ) -> Res<SkinInfo> {
        self.microsoft_account(&uuid)?;
        self.checked_in_path(&file_path, GrantKind::Image)?;
        let path = PathBuf::from(file_path);
        let bytes = {
            let p = path.clone();
            tokio::task::spawn_blocking(move || read_skin_file(&p)).await??
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("skin.png")
            .to_string();
        self.upload_and_refresh(&uuid, &bytes, variant.into(), &name)
            .await
    }

    /// Copy another player's public skin onto this account.
    pub async fn skin_use_username(
        &self,
        uuid: String,
        name: String,
        variant: Option<SkinVariantName>,
    ) -> Res<SkinInfo> {
        self.microsoft_account(&uuid)?;
        let ep = self.inner.opts.auth_endpoints.clone();
        let not_found = || AppError::new("skin.not_found", "No skin found for that player");
        let other = mango_auth::lookup_player_uuid(&ep, name.trim())
            .await
            .map_err(skin_err)?
            .ok_or_else(not_found)?;
        let (bytes, slim) = mango_auth::fetch_public_skin(&ep, &other)
            .await
            .map_err(skin_err)?
            .ok_or_else(not_found)?;
        validate_skin_png(&bytes)?;
        let variant = variant.map_or_else(
            || {
                if slim {
                    SkinVariant::Slim
                } else {
                    SkinVariant::Classic
                }
            },
            Into::into,
        );
        self.upload_and_refresh(&uuid, &bytes, variant, "skin.png")
            .await
    }

    /// Select a cape (`Some(id)`) or hide it (`None`).
    pub async fn skin_set_cape(&self, uuid: String, cape_id: Option<String>) -> Res<SkinInfo> {
        self.microsoft_account(&uuid)?;
        if let Some(id) = &cape_id {
            if !valid_cape_id(id) {
                return Err(AppError::validation("Invalid cape id"));
            }
        }
        let account = self.refresh_session(&uuid, false).await?;
        let ep = self.inner.opts.auth_endpoints.clone();
        mango_auth::set_active_cape(&ep, &account, cape_id.as_deref())
            .await
            .map_err(skin_err)?;
        self.refresh_skin(&uuid, true, true).await?;
        self.build_skin_info(&uuid).await
    }

    // ---- commands: local library ----

    pub async fn skin_library_list(&self) -> Res<Vec<LibrarySkin>> {
        let paths = self.inner.paths.clone();
        Ok(
            tokio::task::spawn_blocking(move || load_index(&paths).iter().map(view).collect())
                .await?,
        )
    }

    pub async fn skin_library_add(
        &self,
        file_path: String,
        name: Option<String>,
        variant: SkinVariantName,
    ) -> Res<LibrarySkin> {
        self.checked_in_path(&file_path, GrantKind::Image)?;
        let svc = self.clone();
        let fallback = Path::new(&file_path)
            .file_stem()
            .and_then(|n| n.to_str())
            .map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let bytes = read_skin_file(Path::new(&file_path))?;
            let _gate = lock(&svc.inner.skin_gate);
            library_add(&svc.inner.paths, name.or(fallback), variant, &bytes)
        })
        .await?
    }

    /// Store the account's current skin in the library.
    pub async fn skin_library_save_current(
        &self,
        uuid: String,
        name: Option<String>,
    ) -> Res<LibrarySkin> {
        let account = self.session_account(&uuid)?;
        let paths = self.inner.paths.clone();
        if account.kind == AuthKind::Microsoft
            && mango_auth::get_cached_skin(&paths.cache(), &uuid)
                .await
                .is_none()
        {
            if let Err(e) = self.refresh_skin(&uuid, false, false).await {
                tracing::debug!("skin not fetched for saving: {}", e.code);
            }
        }
        let cached = mango_auth::get_cached_skin(&paths.cache(), &uuid)
            .await
            .filter(|c| c.source != SkinSource::Default)
            .ok_or_else(|| AppError::new("skin.not_found", "The account has no skin to save"))?;
        let bytes = crate::image_protocol::cached_skin_png(&paths, &uuid)
            .ok_or_else(|| AppError::new("skin.not_found", "The account has no skin to save"))?;
        validate_skin_png(&bytes)?;
        let variant = if cached.slim {
            SkinVariantName::Slim
        } else {
            SkinVariantName::Classic
        };
        let name = name.or(Some(account.name.clone()));
        let svc = self.clone();
        tokio::task::spawn_blocking(move || {
            let _gate = lock(&svc.inner.skin_gate);
            library_add(&svc.inner.paths, name, variant, &bytes)
        })
        .await?
    }

    /// Remove a skin; returns the remaining library.
    pub async fn skin_library_remove(&self, id: String) -> Res<Vec<LibrarySkin>> {
        if !valid_library_id(&id) {
            return Err(AppError::new(
                "skin.not_found",
                "That skin is not in the library",
            ));
        }
        let svc = self.clone();
        tokio::task::spawn_blocking(move || -> Res<Vec<LibrarySkin>> {
            let _gate = lock(&svc.inner.skin_gate);
            let paths = &svc.inner.paths;
            let mut skins = load_index(paths);
            let before = skins.len();
            skins.retain(|e| e.id != id);
            if skins.len() == before {
                return Err(AppError::new(
                    "skin.not_found",
                    "That skin is not in the library",
                ));
            }
            save_index(paths, &skins)?;
            let _ = std::fs::remove_file(library_dir(paths).join(format!("{id}.png")));
            Ok(skins.iter().map(view).collect())
        })
        .await?
    }

    /// Upload a library skin (with its variant) as the account's skin.
    pub async fn skin_library_apply(&self, uuid: String, id: String) -> Res<SkinInfo> {
        self.microsoft_account(&uuid)?;
        let paths = self.inner.paths.clone();
        let (entry, bytes) =
            tokio::task::spawn_blocking(move || library_read(&paths, &id)).await??;
        validate_skin_png(&bytes)?;
        self.upload_and_refresh(
            &uuid,
            &bytes,
            entry.variant.into(),
            &format!("{}.png", entry.id),
        )
        .await
    }
}

fn cape_id(cape: &mango_auth::Texture) -> Option<String> {
    cape.extra
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|id| valid_cape_id(id))
        .map(str::to_string)
}

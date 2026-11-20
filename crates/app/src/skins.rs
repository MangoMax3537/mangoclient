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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// `true` when this answer came from the cache and a refresh is running
    /// in the background; `skin:updated` follows when that refresh found
    /// something different.
    pub stale: bool,
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
pub(crate) fn skin_err(e: AuthError) -> AppError {
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
        AuthError::RateLimited(secs) => AppError::new(
            "skin.rate_limited",
            format!("The skin service is busy, try again in {secs} seconds"),
        ),
        AuthError::CapeNotApplied => AppError::new(
            "skin.cape_not_applied",
            "The skin service did not apply the cape",
        ),
        AuthError::TextureHost => AppError::new(
            "skin.texture_host",
            "The skin service pointed to a texture on an untrusted host",
        ),
        AuthError::Timeout => {
            AppError::new("skin.timeout", "The skin service did not answer in time")
        }
        AuthError::Request(ref r) if r.is_timeout() => {
            AppError::new("skin.timeout", "The skin service did not answer in time")
        }
        AuthError::Request(_) => AppError::new(
            "skin.network",
            "Could not reach the skin service; check the internet connection",
        ),
        other => other.into(),
    }
}

fn skin_timeout() -> AppError {
    AppError::new("skin.timeout", "The skin service did not answer in time")
}

fn offline_refusal() -> AppError {
    skin_err(AuthError::OfflineSkin)
}

/// A name typed for a rename: trimmed, 1..=48 characters, no control characters.
fn checked_name(raw: &str) -> Res<String> {
    let name = raw.trim();
    let bad = |why: &str| AppError::new("skin.invalid_name", format!("Invalid skin name: {why}"));
    if name.is_empty() {
        return Err(bad("empty"));
    }
    if name.chars().any(char::is_control) {
        return Err(bad("control characters"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(bad("too long"));
    }
    Ok(name.to_string())
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

/// Serializes everything that reads or changes one account's skin state, so
/// a background refresh that started before a cape change cannot write its
/// older answer over the newer one, and so concurrent loads share one fetch.
#[derive(Default)]
pub struct SkinFlight {
    lock: Arc<tokio::sync::Mutex<()>>,
    /// Refreshes finished (any kind) / finished with a token.
    done: AtomicU64,
    done_token: AtomicU64,
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

impl AppService {
    // ---- fetching and caching ----

    fn skin_flight(&self, uuid: &str) -> Arc<SkinFlight> {
        lock(&self.inner.skin_flights)
            .entry(uuid.to_string())
            .or_default()
            .clone()
    }

    /// The auth endpoints with the retry budget skin calls use.
    fn skin_ep(&self) -> mango_auth::Endpoints {
        let ep = &self.inner.opts.auth_endpoints;
        mango_auth::Endpoints {
            retries: ep.retries.min(3),
            ..ep.clone()
        }
    }

    /// Longest a whole skin operation may take (every request has its own
    /// timeout; this bounds retries and waits on top).
    fn skin_deadline(&self) -> Duration {
        let ep = &self.inner.opts.auth_endpoints;
        ep.request_timeout * 3 + ep.retry_after_cap
    }

    async fn skin_bounded<T>(&self, fut: impl std::future::Future<Output = Res<T>>) -> Res<T> {
        tokio::time::timeout(self.skin_deadline(), fut)
            .await
            .unwrap_or_else(|_| Err(skin_timeout()))
    }

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
    /// what the UI shows changed (or `force_emit`). Offline accounts: nothing.
    /// Concurrent calls for one account queue up and share the result of the
    /// refresh that finished while they waited.
    pub(crate) async fn refresh_skin(
        &self,
        uuid: &str,
        use_token: bool,
        force_emit: bool,
    ) -> Res<()> {
        let flight = self.skin_flight(uuid);
        let counter = |f: &SkinFlight| {
            if use_token {
                f.done_token.load(Ordering::SeqCst)
            } else {
                f.done.load(Ordering::SeqCst)
            }
        };
        let seen = counter(&flight);
        let guard = flight.lock.clone().lock_owned().await;
        if !force_emit && counter(&flight) != seen {
            return Ok(());
        }
        let r = self.refresh_in_flight(uuid, use_token, force_emit).await;
        drop(guard);
        r
    }

    /// Stale-while-revalidate: start a background refresh unless one is
    /// already running for this account.
    fn spawn_revalidate(&self, uuid: &str) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let flight = self.skin_flight(uuid);
        let Ok(guard) = flight.lock.clone().try_lock_owned() else {
            return;
        };
        let svc = self.clone();
        let uuid = uuid.to_string();
        tokio::spawn(async move {
            if let Err(e) = svc.refresh_in_flight(&uuid, true, false).await {
                tracing::info!("skin revalidation failed: {}", e.code);
            }
            drop(guard);
        });
    }

    /// The refresh itself; the caller holds the account's flight lock.
    async fn refresh_in_flight(&self, uuid: &str, use_token: bool, force_emit: bool) -> Res<()> {
        let flight = self.skin_flight(uuid);
        let started = Instant::now();
        let r = self
            .skin_bounded(self.refresh_inner(uuid, use_token, force_emit))
            .await;
        flight.done.fetch_add(1, Ordering::SeqCst);
        if use_token {
            flight.done_token.fetch_add(1, Ordering::SeqCst);
        }
        tracing::info!(
            ms = elapsed_ms(started),
            use_token,
            outcome = r.as_ref().map_or_else(|e| e.code.as_str(), |()| "ok"),
            "skin refresh"
        );
        r
    }

    async fn refresh_inner(&self, uuid: &str, use_token: bool, force_emit: bool) -> Res<()> {
        let account = self.session_account(uuid)?;
        if account.kind == AuthKind::Offline {
            return Ok(());
        }
        let ep = self.skin_ep();
        let before = self.info_snapshot(uuid).await;
        let first_load = before.is_none();

        let mut working: AuthAccount = account;
        let mut failure: Option<AppError> = None;
        let mut fresh = false;
        if use_token {
            // The profile request and (on a first load) the textures of the
            // stored lists run concurrently: textures are content-addressed,
            // so an unchanged URL needs no second download.
            let stored = working.clone();
            let chain = async {
                let acc = self.refresh_session(uuid, false).await?;
                let t = mango_auth::fetch_profile_textures(&ep, &acc)
                    .await
                    .map_err(skin_err)?;
                Ok::<_, AppError>((acc, t))
            };
            let prewarm = async {
                if first_load && (!stored.skins.is_empty() || !stored.capes.is_empty()) {
                    let mut tex = stored.clone();
                    let capes = std::mem::take(&mut tex.capes);
                    let cache = self.inner.paths.cache();
                    tokio::join!(
                        mango_auth::get_skin(&ep, &cache, &tex),
                        self.fetch_cape_files(uuid, &capes),
                    );
                }
            };
            let (chained, ()) = tokio::join!(chain, prewarm);
            match chained {
                Ok((acc, t)) => {
                    working = acc;
                    working.skins = t.skins.clone();
                    self.store_textures(uuid, t.skins, t.capes).await;
                    fresh = true;
                }
                Err(e) => {
                    tracing::debug!("skin profile not read: {}", e.code);
                    failure = Some(e);
                }
            }
        }
        if !fresh {
            // Possibly stale stored texture urls: ask the session server.
            working.skins.clear();
        }
        self.refresh_images(uuid, &working).await;

        let after = self.info_snapshot(uuid).await;
        if force_emit || before != after {
            self.inner.emit(Event::SkinUpdated(SkinUpdatedEvent {
                uuid: uuid.to_string(),
            }));
        }
        // Nothing usable came out of a failed lookup: the caller must hear why.
        if let Some(e) = failure {
            let usable = after.is_some_and(|i| i.source != SkinSourceName::Default);
            if !usable {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Skin PNG (from `working`'s list or the session server) and every cape
    /// file, concurrently.
    async fn refresh_images(&self, uuid: &str, working: &AuthAccount) {
        let ep = self.skin_ep();
        let capes = self
            .session_account(uuid)
            .map(|a| a.capes.clone())
            .unwrap_or_default();
        let mut for_skin = working.clone();
        // Cape files are cached by `fetch_cape_files`; the skin cache needs none.
        for_skin.capes.clear();
        let cache = self.inner.paths.cache();
        tokio::join!(
            mango_auth::get_skin(&ep, &cache, &for_skin),
            self.fetch_cape_files(uuid, &capes),
        );
    }

    /// What the UI would show now, ignoring `stale` (`None` = nothing cached).
    async fn info_snapshot(&self, uuid: &str) -> Option<SkinInfo> {
        mango_auth::get_cached_skin(&self.inner.paths.cache(), uuid)
            .await
            .as_ref()?;
        self.build_skin_info(uuid, false).await.ok()
    }

    /// Download the capes that are not cached yet, all at once (best effort).
    async fn fetch_cape_files(&self, uuid: &str, capes: &[mango_auth::Texture]) {
        let ep = self.skin_ep();
        let dir = cape_dir(&self.inner.paths, uuid);
        let mut set = tokio::task::JoinSet::new();
        for cape in capes {
            let Some((id, url)) = cape_id(cape).zip(cape.url.clone()) else {
                continue;
            };
            let file = dir.join(format!("{id}.png"));
            if std::fs::metadata(&file).is_ok_and(|m| m.len() > 0) {
                continue;
            }
            let ep = ep.clone();
            set.spawn(async move {
                match mango_auth::fetch_texture_bytes(&ep, &url).await {
                    Ok(bytes)
                        if bytes.len() as u64 <= MAX_SKIN_BYTES
                            && bytes.starts_with(&PNG_MAGIC) =>
                    {
                        let _ = write_atomic(&file, &bytes);
                    }
                    Ok(_) => tracing::debug!("a cape texture was not a usable PNG"),
                    Err(e) => tracing::debug!("cape not downloaded: {}", e.code()),
                }
            });
        }
        while set.join_next().await.is_some() {}
    }

    // ---- commands: current skin ----

    async fn build_skin_info(&self, uuid: &str, stale: bool) -> Res<SkinInfo> {
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
            stale,
        })
    }

    /// The account's skin and capes. With a cache the cached answer returns
    /// at once (`stale: true`) and a background refresh follows (`skin:updated`
    /// when it found something different); only the very first load waits
    /// for the network, and it ends in an error instead of waiting forever.
    pub async fn skin_get(&self, uuid: String) -> Res<SkinInfo> {
        let account = self.session_account(&uuid)?;
        if account.kind == AuthKind::Microsoft {
            let cached = mango_auth::get_cached_skin(&self.inner.paths.cache(), &uuid)
                .await
                .is_some();
            if cached {
                self.spawn_revalidate(&uuid);
                return self.build_skin_info(&uuid, true).await;
            }
            self.refresh_skin(&uuid, true, false).await?;
        }
        self.build_skin_info(&uuid, false).await
    }

    fn microsoft_account(&self, uuid: &str) -> Res<AuthAccount> {
        let account = self.session_account(uuid)?;
        if account.kind == AuthKind::Offline {
            return Err(offline_refusal());
        }
        Ok(account)
    }

    /// The lists a change answered with; one profile request only when the
    /// answer carried none.
    async fn textures_of_answer(
        &self,
        ep: &mango_auth::Endpoints,
        account: &AuthAccount,
        answer: &serde_json::Value,
    ) -> Res<mango_auth::ProfileTextures> {
        match mango_auth::profile_textures(answer) {
            Some(t) => Ok(t),
            None => mango_auth::fetch_profile_textures(ep, account)
                .await
                .map_err(skin_err),
        }
    }

    /// Upload `bytes` as the account's skin, remember the answer and announce it.
    async fn upload_and_refresh(
        &self,
        uuid: &str,
        bytes: &[u8],
        variant: SkinVariant,
        filename: &str,
    ) -> Res<SkinInfo> {
        let started = Instant::now();
        let flight = self.skin_flight(uuid);
        let _guard = flight.lock.clone().lock_owned().await;
        let r = self
            .skin_bounded(async {
                let account = self.refresh_session(uuid, false).await?;
                let ep = self.skin_ep();
                let answer = mango_auth::upload_skin_bytes(&ep, &account, bytes, variant, filename)
                    .await
                    .map_err(skin_err)?;
                let t = self.textures_of_answer(&ep, &account, &answer).await?;
                let mut working = account;
                working.skins = t.skins.clone();
                self.store_textures(uuid, t.skins, t.capes).await;
                self.refresh_images(uuid, &working).await;
                self.inner.emit(Event::SkinUpdated(SkinUpdatedEvent {
                    uuid: uuid.to_string(),
                }));
                self.build_skin_info(uuid, false).await
            })
            .await;
        tracing::info!(
            ms = elapsed_ms(started),
            outcome = r.as_ref().map_or_else(|e| e.code.as_str(), |_| "ok"),
            "skin upload"
        );
        r
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

    /// Select a cape (`Some(id)`) or hide it (`None`). The service answers
    /// with the updated profile: that answer (checked for the requested cape
    /// being ACTIVE) updates the cache, no second round trip.
    pub async fn skin_set_cape(&self, uuid: String, cape_id: Option<String>) -> Res<SkinInfo> {
        self.microsoft_account(&uuid)?;
        if let Some(id) = &cape_id {
            if !valid_cape_id(id) {
                return Err(AppError::validation("Invalid cape id"));
            }
        }
        let started = Instant::now();
        let flight = self.skin_flight(&uuid);
        let _guard = flight.lock.clone().lock_owned().await;
        let r = self
            .skin_bounded(async {
                let account = self.refresh_session(&uuid, false).await?;
                let ep = self.skin_ep();
                let answer = mango_auth::set_active_cape(&ep, &account, cape_id.as_deref())
                    .await
                    .map_err(skin_err)?;
                let t = self.textures_of_answer(&ep, &account, &answer).await?;
                if !mango_auth::cape_applied(&t, cape_id.as_deref()) {
                    return Err(skin_err(AuthError::CapeNotApplied));
                }
                let skins = if t.skins.is_empty() {
                    account.skins.clone()
                } else {
                    t.skins
                };
                self.store_textures(&uuid, skins, t.capes.clone()).await;
                self.fetch_cape_files(&uuid, &t.capes).await;
                self.inner
                    .emit(Event::SkinUpdated(SkinUpdatedEvent { uuid: uuid.clone() }));
                self.build_skin_info(&uuid, false).await
            })
            .await;
        tracing::info!(
            ms = elapsed_ms(started),
            outcome = r.as_ref().map_or_else(|e| e.code.as_str(), |_| "ok"),
            "skin cape change"
        );
        r
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

    /// Rename a saved skin; returns the updated entry.
    pub async fn skin_library_rename(&self, id: String, name: String) -> Res<LibrarySkin> {
        let name = checked_name(&name)?;
        if !valid_library_id(&id) {
            return Err(AppError::new(
                "skin.not_found",
                "That skin is not in the library",
            ));
        }
        let svc = self.clone();
        tokio::task::spawn_blocking(move || -> Res<LibrarySkin> {
            let _gate = lock(&svc.inner.skin_gate);
            let paths = &svc.inner.paths;
            let mut skins = load_index(paths);
            let entry = skins.iter_mut().find(|e| e.id == id).ok_or_else(|| {
                AppError::new("skin.not_found", "That skin is not in the library")
            })?;
            entry.name = name;
            let out = view(entry);
            save_index(paths, &skins)?;
            Ok(out)
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

//! Instance backgrounds: the real title-screen panorama of the instance's
//! Minecraft version (extracted lazily from the installed client jar), a
//! screenshot, an uploaded picture, or none.

use crate::block_icons::{installed_jar, read_entry, release_key, safe_version_dir};
use crate::error::{AppError, Res};
use crate::events::{Event, PanoramaReadyEvent};
use crate::icon_images::{background_png, read_picture, write_atomic, MAX_UPLOAD_BYTES};
use crate::service::{lock, AppService};
use crate::types::ProfileView;
use mango_core::Paths;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Per-face size cap.
pub const MAX_PANORAMA_BYTES: u64 = 8 * 1024 * 1024;
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanoramaSource {
    Own,
    Nearest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanoramaSet {
    pub mc_version: String,
    pub faces: Vec<String>,
    pub source: PanoramaSource,
}

pub(crate) fn panoramas_root(paths: &Paths) -> PathBuf {
    paths.cache().join("panoramas")
}

pub(crate) fn panorama_url(version: &str, n: u8) -> String {
    format!("mangoimg://panorama/{version}/{n}")
}

fn complete(paths: &Paths, version: &str) -> bool {
    safe_version_dir(version)
        && (0..6).all(|n| {
            std::fs::metadata(panoramas_root(paths).join(version).join(format!("{n}.png")))
                .is_ok_and(|m| m.is_file() && m.len() > 0)
        })
}

/// Newest version (by release order, then name) that has all six faces.
fn newest_extracted(paths: &Paths) -> Option<String> {
    let mut found: Vec<(Option<Vec<u32>>, String)> = std::fs::read_dir(panoramas_root(paths))
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|v| complete(paths, v))
        .map(|v| (release_key(&v), v))
        .collect();
    found.sort();
    found.pop().map(|(_, v)| v)
}

fn set_of(version: &str, source: PanoramaSource) -> PanoramaSet {
    PanoramaSet {
        mc_version: version.to_string(),
        faces: (0..6).map(|n| panorama_url(version, n)).collect(),
        source,
    }
}

/// Copy the six whitelisted faces of `jar` to `cache/panoramas/<version>/`
/// without touching their bytes. All or nothing.
pub(crate) fn extract_panoramas(jar: &Path, paths: &Paths, version: &str, force: bool) -> Res<()> {
    if !safe_version_dir(version) {
        return Err(AppError::new("panorama.bad_version", "Unsafe version id"));
    }
    if !force && complete(paths, version) {
        return Ok(());
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar)?)
        .map_err(|e| AppError::new("blocks.jar_unreadable", e.to_string()))?;
    let mut faces = Vec::with_capacity(6);
    for n in 0..6 {
        let entry = format!("assets/minecraft/textures/gui/title/background/panorama_{n}.png");
        let bytes = read_entry(&mut zip, &entry, MAX_PANORAMA_BYTES)?;
        if !bytes.starts_with(&PNG_MAGIC) {
            return Err(AppError::new(
                "panorama.not_png",
                "Panorama face is not a PNG",
            ));
        }
        faces.push(bytes);
    }
    let dir = panoramas_root(paths).join(version);
    for (n, bytes) in faces.iter().enumerate() {
        write_atomic(&dir.join(format!("{n}.png")), bytes)?;
    }
    Ok(())
}

impl AppService {
    /// The panorama for a profile. Extraction runs in the background; the
    /// answer is `None` until something is available, then `panorama:ready`.
    pub async fn panorama_for_profile(&self, id: String) -> Res<Option<PanoramaSet>> {
        let profile = self.profile(&id)?;
        let paths = &self.inner.paths;
        let own = profile.mc_version.as_str();
        if complete(paths, own) {
            return Ok(Some(set_of(own, PanoramaSource::Own)));
        }
        let nearest = newest_extracted(paths);
        let own_jar = installed_jar(paths, own).filter(|_| safe_version_dir(own));
        if own_jar.is_some() || nearest.is_none() {
            self.spawn_panorama_job(id, own_jar.map(|_| own.to_string()));
        }
        Ok(nearest.map(|v| set_of(&v, PanoramaSource::Nearest)))
    }

    /// `own`: extract from that installed version's jar; else from whatever
    /// jar `block_source_jar` finds or downloads.
    fn spawn_panorama_job(&self, profile_id: String, own: Option<String>) {
        if !lock(&self.inner.pano_busy).insert(profile_id.clone()) {
            return;
        }
        let svc = self.clone();
        tokio::spawn(async move {
            let result = svc.run_panorama_job(own).await;
            lock(&svc.inner.pano_busy).remove(&profile_id);
            match result {
                Ok(()) => svc
                    .inner
                    .emit(Event::PanoramaReady(PanoramaReadyEvent { profile_id })),
                Err(e) => tracing::debug!("panorama not extracted: {e}"),
            }
        });
    }

    async fn run_panorama_job(&self, own: Option<String>) -> Res<()> {
        let (version, jar) = match own {
            Some(v) => match installed_jar(&self.inner.paths, &v) {
                Some(jar) => (v, jar),
                None => return Err(AppError::new("panorama.no_jar", "Version is not installed")),
            },
            None => self.block_source_jar().await?,
        };
        let paths = self.inner.paths.clone();
        tokio::task::spawn_blocking(move || extract_panoramas(&jar, &paths, &version, false))
            .await?
    }

    fn bg_view(&self, id: &str) -> Res<ProfileView> {
        let p = self.profile(id)?;
        Ok(crate::blocked::view_of(&self.inner.paths, &p))
    }

    async fn set_background_png(&self, id: String, source: BgSource) -> Res<ProfileView> {
        self.profile(&id)?;
        let dest = self.inner.paths.background_file(&id)?;
        tokio::task::spawn_blocking(move || -> Res<()> {
            let bytes = match source {
                BgSource::File(p) => read_picture(&p)?,
                BgSource::Screenshot(p) => read_screenshot(&p)?,
            };
            write_atomic(&dest, &background_png(&bytes)?)?;
            Ok(())
        })
        .await??;
        self.inner
            .patch_profile(&id, &json!({"background": "image"}))?;
        self.bg_view(&id)
    }

    pub async fn profile_background_set_screenshot(
        &self,
        id: String,
        name: String,
    ) -> Res<ProfileView> {
        let dir = self.screenshot_dir(&id)?;
        if !crate::media::bare_name(&name) || !crate::media::is_screenshot(&name) {
            return Err(AppError::new("invalid_name", "Invalid screenshot name"));
        }
        let file = dir.join(name);
        let instance = self.inner.paths.instance_dir(&id)?;
        let inside = match (
            std::fs::canonicalize(&file),
            std::fs::canonicalize(&instance),
        ) {
            (Ok(f), Ok(i)) => mango_mango::security::contains_path(&i, &f),
            _ => false,
        };
        if !inside {
            return Err(AppError::new("not_found", "Screenshot not found"));
        }
        self.set_background_png(id, BgSource::Screenshot(file))
            .await
    }

    pub async fn profile_background_set_image(
        &self,
        id: String,
        file_path: String,
    ) -> Res<ProfileView> {
        self.set_background_png(id, BgSource::File(PathBuf::from(file_path)))
            .await
    }

    /// Per-instance dim (0..1) and blur (0..24 px) of the background. An
    /// absent field is left as it is; out-of-range values are rejected with
    /// `background.bad_style` (never clamped).
    pub fn profile_background_set_style(
        &self,
        id: String,
        dim: Option<f64>,
        blur: Option<f64>,
    ) -> Res<ProfileView> {
        self.profile(&id)?;
        let ok = |v: Option<f64>, max: f64| match v {
            None => true,
            Some(f) => f.is_finite() && (0.0..=max).contains(&f),
        };
        if !ok(dim, 1.0) || !ok(blur, 24.0) {
            return Err(AppError::new(
                "background.bad_style",
                "dim must be within 0..1 and blur within 0..24",
            ));
        }
        let mut patch = serde_json::Map::new();
        if let Some(d) = dim {
            patch.insert("backgroundDim".into(), json!(d));
        }
        if let Some(b) = blur {
            patch.insert("backgroundBlur".into(), json!(b));
        }
        if !patch.is_empty() {
            self.inner
                .patch_profile(&id, &serde_json::Value::Object(patch))?;
        }
        self.bg_view(&id)
    }

    /// Choose a non-image background. `wallpaper` is required for (and only
    /// for) kind `wallpaper`. Dim/blur are kept. Image backgrounds are set
    /// with `profile_background_set_image` / `_set_screenshot`.
    pub fn profile_background_set_kind(
        &self,
        id: String,
        kind: String,
        wallpaper: Option<String>,
    ) -> Res<ProfileView> {
        self.profile(&id)?;
        let value = match (kind.as_str(), wallpaper) {
            ("auto", None) => json!(null),
            ("none", None) => json!("none"),
            ("panorama", None) => json!("panorama"),
            ("wallpaper", Some(w)) if mango_core::is_wallpaper_id(&w) => {
                json!(format!("wallpaper:{w}"))
            }
            ("wallpaper", _) => {
                return Err(AppError::new(
                    "background.bad_wallpaper",
                    "wallpaper must be 1 to 64 characters of a-z, 0-9 and -",
                ))
            }
            _ => {
                return Err(AppError::new(
                    "background.bad_kind",
                    "kind must be auto, none, panorama or wallpaper",
                ))
            }
        };
        self.inner
            .patch_profile(&id, &json!({"background": value}))?;
        self.bg_view(&id)
    }

    /// Back to automatic, or `none` (no background at all).
    pub fn profile_background_clear(&self, id: String, none: bool) -> Res<ProfileView> {
        self.profile(&id)?;
        match std::fs::remove_file(self.inner.paths.background_file(&id)?) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let value = if none { json!("none") } else { json!(null) };
        // A cleared background also forgets its dim/blur override.
        self.inner.patch_profile(
            &id,
            &json!({"background": value, "backgroundDim": null, "backgroundBlur": null}),
        )?;
        self.bg_view(&id)
    }
}

enum BgSource {
    File(PathBuf),
    Screenshot(PathBuf),
}

/// A screenshot of the instance: a real file (never a link), size-capped.
fn read_screenshot(path: &Path) -> Res<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| AppError::new("not_found", "Screenshot not found"))?;
    if !meta.is_file() {
        return Err(AppError::new("not_found", "Screenshot not found"));
    }
    if meta.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::new(
            "image_too_large",
            "The image is larger than 10 MB",
        ));
    }
    Ok(std::fs::read(path)?)
}

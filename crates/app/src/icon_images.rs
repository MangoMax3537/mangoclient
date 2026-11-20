//! Decoding, cropping and writing of user pictures (instance icons and
//! backgrounds) plus the `profile_icon_*` commands and the modpack logo.

use crate::error::{AppError, Res};
use crate::service::AppService;
use crate::types::ProfileView;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbaImage};
use serde_json::json;
use std::io::Cursor;
use std::path::Path;

/// Icons are stored at this size.
pub const ICON_SIZE: u32 = 256;
/// Largest picture file we read from disk (uploads and screenshots).
pub const MAX_UPLOAD_BYTES: u64 = 10 * 1024 * 1024;
/// Largest remote pack logo.
pub const MAX_LOGO_BYTES: usize = 5 * 1024 * 1024;
pub const BG_MAX_W: u32 = 1920;
pub const BG_MAX_H: u32 = 1080;

const PICTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

fn decode_error(msg: impl std::fmt::Display) -> AppError {
    AppError::new("image_decode", format!("Could not read the image: {msg}"))
}

/// Decode with hard limits; only the formats the app accepts.
pub(crate) fn decode_image(bytes: &[u8], only: Option<ImageFormat>) -> Res<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(decode_error)?;
    let format = reader.format();
    let allowed = match only {
        Some(f) => format == Some(f),
        None => matches!(
            format,
            Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif)
        ),
    };
    if !allowed {
        return Err(decode_error("unsupported format"));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(decode_error)
}

pub(crate) fn encode_png(img: &RgbaImage) -> Res<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| AppError::new("image_encode", e.to_string()))?;
    Ok(out.into_inner())
}

/// Center-crop to a square and resize to 256x256.
pub(crate) fn icon_png(bytes: &[u8]) -> Res<Vec<u8>> {
    let rgba = decode_image(bytes, None)?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let side = w.min(h);
    if side == 0 {
        return Err(decode_error("empty image"));
    }
    let square = imageops::crop_imm(&rgba, (w - side) / 2, (h - side) / 2, side, side).to_image();
    let filter = if side > ICON_SIZE {
        FilterType::Lanczos3
    } else {
        FilterType::Triangle
    };
    let small = if side == ICON_SIZE {
        square
    } else {
        imageops::resize(&square, ICON_SIZE, ICON_SIZE, filter)
    };
    encode_png(&small)
}

/// Fit into 1920x1080 (never upscaled), PNG.
pub(crate) fn background_png(bytes: &[u8]) -> Res<Vec<u8>> {
    let img = decode_image(bytes, None)?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(decode_error("empty image"));
    }
    let img = if w > BG_MAX_W || h > BG_MAX_H {
        img.resize(BG_MAX_W, BG_MAX_H, FilterType::Lanczos3)
    } else {
        img
    };
    encode_png(&img.to_rgba8())
}

/// Read a picture the user pointed at: extension, regular file, size cap.
pub(crate) fn read_picture(path: &Path) -> Res<Vec<u8>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !PICTURE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::new(
            "image_unsupported_type",
            "Only png, jpg, webp and gif images are supported",
        ));
    }
    let meta =
        std::fs::metadata(path).map_err(|_| AppError::new("not_found", "Image file not found"))?;
    if !meta.is_file() {
        return Err(AppError::new("not_found", "Image file not found"));
    }
    if meta.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::new(
            "image_too_large",
            "The image is larger than 10 MB",
        ));
    }
    Ok(std::fs::read(path)?)
}

/// Write through a temp file in the same folder, then rename.
pub(crate) fn write_atomic(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(format!(".part-{}", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    if let Err(e) = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dest)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

impl AppService {
    fn view(&self, id: &str) -> Res<ProfileView> {
        let p = self.profile(id)?;
        Ok(self.inner.view(&p))
    }

    pub fn profile_icon_set_block(&self, id: String, block: String) -> Res<ProfileView> {
        self.profile(&id)?;
        if !crate::block_icons::is_known_block(&block) {
            return Err(AppError::new("block_unknown", "Unknown block"));
        }
        let cover = self.inner.paths.cover_file(&id)?;
        match std::fs::remove_file(&cover) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        self.inner.patch_profile(
            &id,
            &json!({"cover": false, "iconBlock": block, "iconSource": null}),
        )?;
        self.view(&id)
    }

    pub async fn profile_icon_set_image(&self, id: String, file_path: String) -> Res<ProfileView> {
        self.profile(&id)?;
        self.checked_in_path(&file_path, crate::path_grants::GrantKind::Image)?;
        let dest = self.inner.paths.cover_file(&id)?;
        tokio::task::spawn_blocking(move || -> Res<()> {
            let png = icon_png(&read_picture(Path::new(&file_path))?)?;
            write_atomic(&dest, &png)?;
            Ok(())
        })
        .await??;
        self.inner.patch_profile(
            &id,
            &json!({"cover": true, "iconBlock": null, "iconSource": "custom"}),
        )?;
        self.view(&id)
    }

    pub fn profile_icon_clear(&self, id: String) -> Res<ProfileView> {
        self.profile(&id)?;
        let cover = self.inner.paths.cover_file(&id)?;
        match std::fs::remove_file(&cover) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        self.inner.patch_profile(
            &id,
            &json!({"cover": false, "iconBlock": null, "iconSource": null}),
        )?;
        self.view(&id)
    }

    /// Best effort: save a modpack's logo as the instance icon. Failures are
    /// logged, never returned.
    pub(crate) async fn apply_pack_logo(&self, profile_id: &str, url: Option<&str>) {
        let Some(url) = url else { return };
        if let Err(e) = self.try_pack_logo(profile_id, url).await {
            tracing::warn!("modpack logo not saved: {e}");
        }
    }

    async fn try_pack_logo(&self, profile_id: &str, url: &str) -> Res<()> {
        self.check_icon_host(url)?;
        let bytes = mango_net::get_bytes(url, &self.inner.opts.fetch).await?;
        self.save_pack_logo(profile_id, bytes).await
    }

    /// Best effort: a pack's own logo file (an import's `icon.png`).
    pub(crate) async fn apply_pack_logo_bytes(&self, profile_id: &str, bytes: Vec<u8>) {
        if let Err(e) = self.save_pack_logo(profile_id, bytes).await {
            tracing::warn!("modpack logo not saved: {e}");
        }
    }

    async fn save_pack_logo(&self, profile_id: &str, bytes: Vec<u8>) -> Res<()> {
        if bytes.len() > MAX_LOGO_BYTES {
            return Err(AppError::new("image_too_large", "Logo is too large"));
        }
        let dest = self.inner.paths.cover_file(profile_id)?;
        let pack = self.inner.paths.pack_logo_file(profile_id)?;
        tokio::task::spawn_blocking(move || -> Res<()> {
            let png = icon_png(&bytes)?;
            write_atomic(&pack, &png)?;
            write_atomic(&dest, &png)?;
            Ok(())
        })
        .await??;
        self.inner.patch_profile(
            profile_id,
            &json!({"cover": true, "iconBlock": null, "iconSource": "modpack"}),
        )?;
        Ok(())
    }

    /// Restore the saved modpack logo as the image icon.
    pub async fn profile_icon_use_modpack_logo(&self, id: String) -> Res<ProfileView> {
        self.profile(&id)?;
        let pack = self.inner.paths.pack_logo_file(&id)?;
        let dest = self.inner.paths.cover_file(&id)?;
        tokio::task::spawn_blocking(move || -> Res<()> {
            let bytes = match std::fs::read(&pack) {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(AppError::new(
                        "icon.no_modpack_logo",
                        "This instance has no saved modpack logo",
                    ))
                }
                Err(e) => return Err(e.into()),
            };
            write_atomic(&dest, &bytes)?;
            Ok(())
        })
        .await??;
        self.inner.patch_profile(
            &id,
            &json!({"cover": true, "iconBlock": null, "iconSource": "modpack"}),
        )?;
        self.view(&id)
    }
}

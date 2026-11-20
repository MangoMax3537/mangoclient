//! The `mangoimg://` image protocol, as pure functions.
//!
//! Supported URLs (host form `mangoimg://<kind>/<arg>`; the Windows/Android
//! form `http(s)://mangoimg.localhost/<kind>/<arg>` is accepted too):
//!
//! | URL | serves |
//! |---|---|
//! | `cover/<uuid>` | `covers/<uuid>.png` |
//! | `f/<base64url abs path>` | an image under `covers/`, `cache/mod-icons/`, `cache/server-icons/` or a profile's `screenshots/` |
//! | `head/<uuid>` | the 8x8 face (+ hat) of the cached skin, 64x64 PNG |
//! | `block/<id>/<top\|left\|right>` | a face of the block gallery, `cache/block-icons/` |
//! | `panorama/<version>/<0..5>` | a title-screen panorama face, `cache/panoramas/` |
//! | `world/<uuid>/<folder>` | `icon.png` of a world: `instances/<uuid>/saves/<folder>/icon.png`, folder percent-encoded |
//! | `icon/<base64url https url>` | a remote Modrinth image once cached under `cache/mod-icons/` |
//!
//! Only `png|jpg|jpeg|webp|gif` files are ever served, and only when their
//! canonical real path lies inside the canonical real path of an approved
//! root (no traversal, no sibling prefixes, no escaping symlinks).

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use mango_core::{paths::is_canonical_uuid, Paths};
use mango_mango::security::{approved_image_path, contains_path, IMAGE_EXTENSIONS};
use sha1::{Digest, Sha1};
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

/// Largest skin PNG we are willing to decode.
const MAX_SKIN_BYTES: usize = 2 * 1024 * 1024;
/// Largest `cache/skins/<uuid>.json` we read.
const MAX_SKIN_JSON_BYTES: u64 = 8 * 1024 * 1024;
/// Output size of a rendered head.
pub const HEAD_SIZE: u32 = 64;

/// The `Access-Control-Allow-Origin` value for a `mangoimg` response: the
/// request's `Origin` only when it is the app's own webview origin (needed
/// for WebGL skin textures), otherwise none. Never `*`.
pub fn cors_allowed_origin(origin: Option<&str>) -> Option<String> {
    const APP: &[&str] = &[
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
    ];
    #[cfg(debug_assertions)]
    const DEV: &[&str] = &["http://localhost:5173"];
    #[cfg(not(debug_assertions))]
    const DEV: &[&str] = &[];
    let o = origin?;
    (APP.contains(&o) || DEV.contains(&o)).then(|| o.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Image {
    /// A validated real path; read it and serve it.
    File { path: PathBuf, mime: &'static str },
    /// Rendered bytes (heads).
    Bytes { data: Vec<u8>, mime: &'static str },
}

impl Image {
    pub fn mime(&self) -> &'static str {
        match self {
            Image::File { mime, .. } | Image::Bytes { mime, .. } => mime,
        }
    }
}

/// A remote image that is not in the cache yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteImage {
    pub url: String,
    /// Cache path without extension (`cache/mod-icons/remote-<sha1>`).
    pub stem: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("not a mangoimg url")]
    BadUrl,
    #[error("path is not an approved image")]
    Forbidden,
    #[error("image not found")]
    NotFound,
    #[error("remote image is not cached")]
    NeedsFetch(RemoteImage),
}

impl ImageError {
    /// HTTP status for the protocol handler.
    pub fn status(&self) -> u16 {
        match self {
            ImageError::BadUrl => 400,
            ImageError::Forbidden => 403,
            ImageError::NotFound | ImageError::NeedsFetch(_) => 404,
        }
    }
}

// ---- URL builders (what the service puts into the JSON) ----

pub fn cover_url(profile_id: &str) -> String {
    format!("mangoimg://cover/{profile_id}")
}

pub fn head_url(uuid: &str) -> String {
    format!("mangoimg://head/{uuid}")
}

pub fn file_url(file: &Path) -> String {
    format!(
        "mangoimg://f/{}",
        URL_SAFE_NO_PAD.encode(file.to_string_lossy().as_bytes())
    )
}

pub fn remote_url(url: &str) -> String {
    format!("mangoimg://icon/{}", URL_SAFE_NO_PAD.encode(url.as_bytes()))
}

pub fn mime_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return None,
    })
}

/// Magic-byte sniff; returns the file extension.
pub fn sniff_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("png")
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else {
        None
    }
}

// ---- resolution ----

/// `mangoimg://kind/arg` (either spelling) -> `(kind, arg)`.
fn split_url(url: &str) -> Option<(&str, &str)> {
    let rest = [
        "mangoimg://",
        "http://mangoimg.localhost/",
        "https://mangoimg.localhost/",
    ]
    .iter()
    .find_map(|p| url.strip_prefix(p))?;
    if rest.contains(['?', '#', '\\', '\0']) {
        return None;
    }
    let rest = rest.trim_end_matches('/');
    let (kind, arg) = rest.split_once('/')?;
    // `block/<id>/<face>` and `panorama/<version>/<n>` carry two segments.
    let segments = if matches!(kind, "block" | "panorama" | "world") {
        2
    } else {
        1
    };
    (!kind.is_empty()
        && !arg.is_empty()
        && arg.split('/').count() == segments
        && arg.split('/').all(|s| !s.is_empty()))
    .then_some((kind, arg))
}

/// Resolve a `mangoimg://` URL. Never touches the network and never writes.
pub fn resolve(paths: &Paths, url: &str) -> Result<Image, ImageError> {
    let (kind, arg) = split_url(url).ok_or(ImageError::BadUrl)?;
    match kind {
        "cover" => {
            // `<uuid>-bg` is the instance background.
            let file = match arg.strip_suffix("-bg") {
                Some(id) => paths.background_file(id),
                None => paths.cover_file(arg),
            }
            .map_err(|_| ImageError::BadUrl)?;
            approve(&file, &[paths.covers()])
        }
        "block" => {
            let (id, face) = arg.split_once('/').ok_or(ImageError::BadUrl)?;
            let valid_id = !id.is_empty()
                && id.len() <= 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !valid_id || !matches!(face, "top" | "left" | "right") {
                return Err(ImageError::BadUrl);
            }
            let root = crate::block_icons::blocks_root(paths);
            let current = std::fs::read(root.join("current.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|v| v.get("mcVersion")?.as_str().map(str::to_string))
                .filter(|v| crate::block_icons::safe_version_dir(v))
                .ok_or(ImageError::NotFound)?;
            let file = root.join(current).join(id).join(format!("{face}.png"));
            approve(&file, &[root])
        }
        "panorama" => {
            let (version, n) = arg.split_once('/').ok_or(ImageError::BadUrl)?;
            if !crate::block_icons::safe_version_dir(version)
                || !matches!(n, "0" | "1" | "2" | "3" | "4" | "5")
            {
                return Err(ImageError::BadUrl);
            }
            let root = crate::backgrounds::panoramas_root(paths);
            approve(&root.join(version).join(format!("{n}.png")), &[root])
        }
        "f" => {
            let raw = URL_SAFE_NO_PAD
                .decode(arg.trim_end_matches('='))
                .map_err(|_| ImageError::BadUrl)?;
            let text = String::from_utf8(raw).map_err(|_| ImageError::BadUrl)?;
            let file = PathBuf::from(text);
            if !file.is_absolute()
                || file
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            {
                return Err(ImageError::Forbidden);
            }
            let mut roots = vec![
                paths.covers(),
                mod_icons_dir(paths),
                server_icons_dir(paths),
            ];
            roots.extend(screenshots_root(paths, &file));
            approve(&file, &roots)
        }
        "world" => resolve_world_icon(paths, arg),
        "head" => render_head_for(paths, arg),
        "icon" => {
            let raw = URL_SAFE_NO_PAD
                .decode(arg.trim_end_matches('='))
                .map_err(|_| ImageError::BadUrl)?;
            let remote = String::from_utf8(raw).map_err(|_| ImageError::BadUrl)?;
            resolve_remote(paths, &remote)
        }
        _ => Err(ImageError::BadUrl),
    }
}

pub fn mod_icons_dir(paths: &Paths) -> PathBuf {
    paths.cache().join("mod-icons")
}

pub fn server_icons_dir(paths: &Paths) -> PathBuf {
    paths.cache().join("server-icons")
}

fn approve(file: &Path, roots: &[PathBuf]) -> Result<Image, ImageError> {
    let mime = mime_for(file).ok_or(ImageError::Forbidden)?;
    match approved_image_path(file, roots) {
        Some(path) => Ok(Image::File { path, mime }),
        None if file.exists() => Err(ImageError::Forbidden),
        None => Err(ImageError::NotFound),
    }
}

/// `<instances>/<uuid>/screenshots` when `file` lies (lexically) in one and
/// that directory really is inside the instances folder (a `screenshots`
/// symlink pointing elsewhere does not count).
fn screenshots_root(paths: &Paths, file: &Path) -> Option<PathBuf> {
    let instances = paths.instances();
    let rel = file.strip_prefix(&instances).ok()?;
    let mut parts = rel.components();
    let Component::Normal(id) = parts.next()? else {
        return None;
    };
    if !is_canonical_uuid(id.to_str()?) {
        return None;
    }
    let Component::Normal(dir) = parts.next()? else {
        return None;
    };
    if dir != "screenshots" {
        return None;
    }
    let root = instances.join(id).join("screenshots");
    let real_root = std::fs::canonicalize(&root).ok()?;
    let real_instances = std::fs::canonicalize(&instances).ok()?;
    contains_path(&real_instances, &real_root).then_some(root)
}

// ---- world icons ----

/// `<uuid>/<percent-encoded folder>` -> that world's `icon.png`. The folder
/// must be one plain name, nothing on the way may be a symlink, and the
/// real path must stay inside the instance's real `saves` folder (which in
/// turn must lie inside the real instances folder).
fn resolve_world_icon(paths: &Paths, arg: &str) -> Result<Image, ImageError> {
    let (id, folder) = arg.split_once('/').ok_or(ImageError::BadUrl)?;
    let folder = crate::worlds::decode_segment(folder).ok_or(ImageError::BadUrl)?;
    if !is_canonical_uuid(id) || !crate::worlds::safe_folder_name(&folder) {
        return Err(ImageError::BadUrl);
    }
    let saves = paths
        .instance_dir(id)
        .map_err(|_| ImageError::BadUrl)?
        .join("saves");
    let icon = saves.join(&folder).join("icon.png");
    let is_link = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    if is_link(&saves) || is_link(&saves.join(&folder)) || is_link(&icon) {
        return Err(ImageError::Forbidden);
    }
    let too_big =
        std::fs::symlink_metadata(&icon).is_ok_and(|m| m.len() > crate::worlds::MAX_WORLD_ICON);
    if too_big {
        return Err(ImageError::Forbidden);
    }
    let real_saves = std::fs::canonicalize(&saves).map_err(|_| ImageError::NotFound)?;
    let real_instances =
        std::fs::canonicalize(paths.instances()).map_err(|_| ImageError::NotFound)?;
    if !contains_path(&real_instances, &real_saves) {
        return Err(ImageError::Forbidden);
    }
    approve(&icon, &[saves])
}

// ---- remote icons ----

/// Where a remote image lives once fetched (extension appended by the fetcher).
pub fn remote_stem(paths: &Paths, url: &str) -> PathBuf {
    let hash = hex::encode(Sha1::digest(url.as_bytes()));
    mod_icons_dir(paths).join(format!("remote-{hash}"))
}

fn resolve_remote(paths: &Paths, remote: &str) -> Result<Image, ImageError> {
    let parsed = url::Url::parse(remote).map_err(|_| ImageError::BadUrl)?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(ImageError::BadUrl);
    }
    let stem = remote_stem(paths, remote);
    let roots = [mod_icons_dir(paths)];
    for ext in IMAGE_EXTENSIONS {
        let file = stem.with_extension(ext);
        // `with_extension` would replace a dot in the hash name; the stem has none.
        if let Some(path) = approved_image_path(&file, &roots) {
            let mime = mime_for(&path).ok_or(ImageError::Forbidden)?;
            return Ok(Image::File { path, mime });
        }
    }
    Err(ImageError::NeedsFetch(RemoteImage {
        url: remote.to_string(),
        stem,
    }))
}

// ---- heads ----

/// Java's default-skin rule: odd `UUID.hashCode()` is Alex, even is Steve.
pub fn is_alex(uuid: &str) -> bool {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect();
    let Ok(v) = u128::from_str_radix(&hex, 16) else {
        return false;
    };
    let (hi, lo) = ((v >> 64) as u64, v as u64);
    let hilo = hi ^ lo;
    (((hilo >> 32) as u32) ^ (hilo as u32)) & 1 == 1
}

/// The bundled fallback skin. Only Steve ships today (`mango-auth`); Alex
/// uses the same bytes until the asset exists.
fn default_skin(uuid: &str) -> &'static [u8] {
    let _alex = is_alex(uuid);
    mango_auth::default_skin_png()
}

fn valid_head_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

fn cached_skin_png(paths: &Paths, uuid: &str) -> Option<Vec<u8>> {
    let file = paths.cache().join("skins").join(format!("{uuid}.json"));
    let meta = std::fs::metadata(&file).ok()?;
    if !meta.is_file() || meta.len() > MAX_SKIN_JSON_BYTES {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).ok()?).ok()?;
    let skin = json.get("skin")?.as_str()?;
    let b64 = skin.strip_prefix("data:image/png;base64,")?;
    let png = STANDARD.decode(b64).ok()?;
    (png.len() <= MAX_SKIN_BYTES).then_some(png)
}

fn render_head_for(paths: &Paths, uuid: &str) -> Result<Image, ImageError> {
    if !valid_head_id(uuid) {
        return Err(ImageError::BadUrl);
    }
    let rendered = cached_skin_png(paths, uuid)
        .and_then(|png| render_head(&png).ok())
        .or_else(|| render_head(default_skin(uuid)).ok())
        .ok_or(ImageError::NotFound)?;
    Ok(Image::Bytes {
        data: rendered,
        mime: "image/png",
    })
}

#[derive(Debug, thiserror::Error)]
pub enum HeadError {
    #[error("skin decode: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("head encode: {0}")]
    Encode(#[from] png::EncodingError),
}

/// Decoded RGBA8 skin.
struct Skin {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Skin {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

fn decode_skin(bytes: &[u8]) -> Result<Skin, png::DecodingError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let (w, h) = {
        let info = reader.info();
        (info.width, info.height)
    };
    // Skins are 64 wide (x1 scale) up to HD multiples; refuse anything else
    // before allocating.
    if !(64..=1024).contains(&w) || w % 64 != 0 || h < w / 4 || h > w {
        return Err(png::DecodingError::LimitsExceeded);
    }
    let size = reader
        .output_buffer_size()
        .ok_or(png::DecodingError::LimitsExceeded)?;
    let mut buf = vec![0u8; size];
    let frame = reader.next_frame(&mut buf)?;
    let channels = frame.color_type.samples();
    let data = &buf[..frame.buffer_size()];
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in data.chunks_exact(channels) {
        match frame.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(px),
            png::ColorType::Rgb => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            png::ColorType::GrayscaleAlpha => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            png::ColorType::Grayscale | png::ColorType::Indexed => {
                rgba.extend_from_slice(&[px[0], px[0], px[0], 255])
            }
        }
    }
    if rgba.len() != (w * h * 4) as usize {
        return Err(png::DecodingError::LimitsExceeded);
    }
    Ok(Skin { w, h, rgba })
}

/// Face (8,8 8x8) with the hat layer (40,8 8x8) blended on top, each source
/// pixel scaled to 8x8 with nearest-neighbour: a 64x64 PNG.
pub fn render_head(skin_png: &[u8]) -> Result<Vec<u8>, HeadError> {
    let skin = decode_skin(skin_png)?;
    let s = skin.w / 64;
    if skin.h < 16 * s {
        return Err(png::DecodingError::LimitsExceeded.into());
    }
    let scale = HEAD_SIZE / 8;
    let mut out = Vec::with_capacity((HEAD_SIZE * HEAD_SIZE * 4) as usize);
    for oy in 0..HEAD_SIZE {
        for ox in 0..HEAD_SIZE {
            let (fx, fy) = (ox / scale, oy / scale);
            let face = skin.px((8 + fx) * s, (8 + fy) * s);
            let hat = skin.px((40 + fx) * s, (8 + fy) * s);
            out.extend_from_slice(&blend(face, hat));
        }
    }
    let mut encoded = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut encoded, HEAD_SIZE, HEAD_SIZE);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header()?;
        writer.write_image_data(&out)?;
    }
    Ok(encoded)
}

fn blend(face: [u8; 4], hat: [u8; 4]) -> [u8; 4] {
    let a = u32::from(hat[3]);
    if a == 0 {
        return face;
    }
    if a == 255 {
        return hat;
    }
    let mix = |f: u8, h: u8| ((u32::from(h) * a + u32::from(f) * (255 - a)) / 255) as u8;
    [
        mix(face[0], hat[0]),
        mix(face[1], hat[1]),
        mix(face[2], hat[2]),
        face[3].max(hat[3]),
    ]
}

#[cfg(test)]
mod cors_tests {
    use super::cors_allowed_origin;

    #[test]
    fn only_the_app_origin_is_echoed_never_a_wildcard() {
        assert_eq!(
            cors_allowed_origin(Some("tauri://localhost")).as_deref(),
            Some("tauri://localhost")
        );
        assert_eq!(
            cors_allowed_origin(Some("http://tauri.localhost")).as_deref(),
            Some("http://tauri.localhost")
        );
        assert_eq!(cors_allowed_origin(Some("https://evil.example")), None);
        assert_eq!(cors_allowed_origin(Some("*")), None);
        assert_eq!(cors_allowed_origin(None), None);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const UUID: &str = "12345678-1234-4234-8234-123456789abc";

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        paths.ensure_dirs().unwrap();
        (tmp, paths)
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"\x89PNG data").unwrap();
    }

    fn ok_path(r: Result<Image, ImageError>) -> PathBuf {
        match r.unwrap() {
            Image::File { path, .. } => path,
            other => panic!("expected a file, got {other:?}"),
        }
    }

    #[test]
    fn url_forms_and_builders() {
        let (_t, paths) = setup();
        let cover = paths.cover_file(UUID).unwrap();
        touch(&cover);
        let real = std::fs::canonicalize(&cover).unwrap();
        for url in [
            format!("mangoimg://cover/{UUID}"),
            format!("mangoimg://cover/{UUID}/"),
            format!("http://mangoimg.localhost/cover/{UUID}"),
            format!("https://mangoimg.localhost/cover/{UUID}"),
        ] {
            assert_eq!(ok_path(resolve(&paths, &url)), real, "{url}");
        }
        for bad in [
            "".to_string(),
            "mangoimg://".to_string(),
            "mangoimg://cover".to_string(),
            format!("mangoimg://cover/{UUID}?x=1"),
            format!("mangoimg://cover/{UUID}#x"),
            format!("mangoimg://cover/{UUID}/extra"),
            format!("mangoimg://cover/{UUID}\\x"),
            format!("https://evil.example/cover/{UUID}"),
            format!("file:///cover/{UUID}"),
            format!("mangoimg://nope/{UUID}"),
        ] {
            assert_eq!(
                resolve(&paths, &bad).unwrap_err(),
                ImageError::BadUrl,
                "{bad}"
            );
        }
        assert_eq!(cover_url(UUID), format!("mangoimg://cover/{UUID}"));
        assert_eq!(head_url(UUID), format!("mangoimg://head/{UUID}"));
    }

    #[test]
    fn cover_table() {
        let (_t, paths) = setup();
        assert_eq!(
            resolve(&paths, &format!("mangoimg://cover/{UUID}")).unwrap_err(),
            ImageError::NotFound
        );
        for bad in ["../x", "nope", "12345678123412341234123456789abc", "%2e%2e"] {
            assert_eq!(
                resolve(&paths, &format!("mangoimg://cover/{bad}")).unwrap_err(),
                ImageError::BadUrl,
                "{bad}"
            );
        }
    }

    #[test]
    fn file_urls_accept_only_approved_roots() {
        let (tmp, paths) = setup();
        let good = [
            paths.covers().join("a.png"),
            mod_icons_dir(&paths).join("b.PNG"),
            mod_icons_dir(&paths).join("nested/c.webp"),
            server_icons_dir(&paths).join("d.gif"),
            paths.instance_dir(UUID).unwrap().join("screenshots/e.jpg"),
            paths.instance_dir(UUID).unwrap().join("screenshots/f.jpeg"),
        ];
        for f in &good {
            touch(f);
            let got = ok_path(resolve(&paths, &file_url(f)));
            assert_eq!(got, std::fs::canonicalize(f).unwrap());
        }
        let mime = |f: &Path| resolve(&paths, &file_url(f)).unwrap().mime();
        assert_eq!(mime(&good[0]), "image/png");
        assert_eq!(mime(&good[2]), "image/webp");
        assert_eq!(mime(&good[3]), "image/gif");
        assert_eq!(mime(&good[4]), "image/jpeg");

        let outside = tmp.path().join("secret.png");
        touch(&outside);
        let evil_sibling = paths.cache().join("mod-icons-evil/x.png");
        touch(&evil_sibling);
        let other_dir = paths.instance_dir(UUID).unwrap().join("saves/x.png");
        touch(&other_dir);
        let not_image = mod_icons_dir(&paths).join("notes.txt");
        touch(&not_image);
        let bad_instance = paths.instances().join("not-a-uuid/screenshots/x.png");
        touch(&bad_instance);
        let traversal = mod_icons_dir(&paths).join("../../secret.png");
        let rejected = [
            outside,
            evil_sibling,
            other_dir,
            not_image,
            bad_instance,
            traversal,
            PathBuf::from("relative/x.png"),
            PathBuf::from("/etc/passwd"),
        ];
        for f in &rejected {
            assert!(
                matches!(
                    resolve(&paths, &file_url(f)),
                    Err(ImageError::Forbidden | ImageError::NotFound)
                ),
                "{}",
                f.display()
            );
        }
        // Missing but approved location: not found, not forbidden.
        assert_eq!(
            resolve(&paths, &file_url(&paths.covers().join("gone.png"))).unwrap_err(),
            ImageError::NotFound
        );
        // Garbage payloads.
        for bad in ["mangoimg://f/%%%", "mangoimg://f/AAAA"] {
            assert!(resolve(&paths, bad).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_that_escape_are_rejected() {
        use std::os::unix::fs::symlink;
        let (tmp, paths) = setup();
        let secret = tmp.path().join("outside/secret.png");
        touch(&secret);
        // A link inside an approved root pointing out of it.
        let link = mod_icons_dir(&paths).join("link.png");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&secret, &link).unwrap();
        assert_eq!(
            resolve(&paths, &file_url(&link)).unwrap_err(),
            ImageError::Forbidden
        );
        // A linked directory.
        let dir_link = paths.covers().join("dir");
        symlink(tmp.path().join("outside"), &dir_link).unwrap();
        assert_eq!(
            resolve(&paths, &file_url(&dir_link.join("secret.png"))).unwrap_err(),
            ImageError::Forbidden
        );
        // A `screenshots` directory that leads out of the instances folder.
        let inst = paths.instance_dir(UUID).unwrap();
        std::fs::create_dir_all(&inst).unwrap();
        symlink(tmp.path().join("outside"), inst.join("screenshots")).unwrap();
        assert!(resolve(&paths, &file_url(&inst.join("screenshots/secret.png"))).is_err());
        // A link that stays inside the root is fine.
        let real = server_icons_dir(&paths).join("real.png");
        touch(&real);
        let inner = server_icons_dir(&paths).join("alias.png");
        symlink(&real, &inner).unwrap();
        assert_eq!(
            ok_path(resolve(&paths, &file_url(&inner))),
            std::fs::canonicalize(&real).unwrap()
        );
    }

    /// 64x64 skin: face red, hat pixel (0,0) opaque blue, (1,0) half green.
    fn skin_png(scale: u32) -> Vec<u8> {
        let (w, h) = (64 * scale, 64 * scale);
        let mut px = vec![0u8; (w * h * 4) as usize];
        let mut set = |x: u32, y: u32, c: [u8; 4]| {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&c);
        };
        for y in 8..16 {
            for x in 8..16 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        set(x * scale + dx, y * scale + dy, [200, 0, 0, 255]);
                    }
                }
            }
        }
        for dy in 0..scale {
            for dx in 0..scale {
                set(40 * scale + dx, 8 * scale + dy, [0, 0, 255, 255]);
                set(41 * scale + dx, 8 * scale + dy, [0, 200, 0, 128]);
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        out
    }

    fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut r = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let f = r.next_frame(&mut buf).unwrap();
        (f.width, f.height, buf[..f.buffer_size()].to_vec())
    }

    fn at(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * HEAD_SIZE + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    #[test]
    fn head_is_face_plus_hat_scaled_8x() {
        for scale in [1, 2] {
            let head = render_head(&skin_png(scale)).unwrap();
            let (w, h, px) = decode_rgba(&head);
            assert_eq!((w, h), (64, 64));
            // Hat pixel (0,0) opaque blue covers the face's first 8x8 block.
            assert_eq!(at(&px, 0, 0), [0, 0, 255, 255]);
            assert_eq!(at(&px, 7, 7), [0, 0, 255, 255]);
            // Half-transparent green blends with red.
            let b = at(&px, 8, 0);
            assert!(b[0] > 90 && b[0] < 110 && b[1] > 90 && b[1] < 110, "{b:?}");
            assert_eq!(b[3], 255);
            // Plain face elsewhere, nearest-neighbour: no gradients.
            assert_eq!(at(&px, 16, 0), [200, 0, 0, 255]);
            assert_eq!(at(&px, 63, 63), [200, 0, 0, 255]);
        }
    }

    #[test]
    fn head_rejects_bad_skins() {
        assert!(render_head(b"not a png").is_err());
        // 32x32 is not a skin.
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 32, 32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&vec![0; 32 * 32 * 4])
            .unwrap();
        assert!(render_head(&out).is_err());
    }

    #[test]
    fn legacy_64x32_skins_and_rgb_render() {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 64, 32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&vec![50; 64 * 32 * 3])
            .unwrap();
        let (_, _, px) = decode_rgba(&render_head(&out).unwrap());
        assert_eq!(at(&px, 3, 3), [50, 50, 50, 255]);
    }

    #[test]
    fn head_from_cached_skin_and_fallbacks() {
        let (_t, paths) = setup();
        let skins = paths.cache().join("skins");
        std::fs::create_dir_all(&skins).unwrap();
        let data = format!("data:image/png;base64,{}", STANDARD.encode(skin_png(1)));
        std::fs::write(
            skins.join(format!("{UUID}.json")),
            serde_json::json!({"skin": data, "slim": false, "source": "account"}).to_string(),
        )
        .unwrap();
        let Image::Bytes { data, mime } =
            resolve(&paths, &format!("mangoimg://head/{UUID}")).unwrap()
        else {
            panic!("head must be rendered bytes");
        };
        assert_eq!(mime, "image/png");
        let (_, _, px) = decode_rgba(&data);
        assert_eq!(at(&px, 0, 0), [0, 0, 255, 255]);

        // No cache, broken cache and an offline-style id fall back to Steve.
        let steve = render_head(mango_auth::default_skin_png()).unwrap();
        for id in ["abcdef0123456789abcdef0123456789", UUID] {
            if id == UUID {
                std::fs::write(skins.join(format!("{id}.json")), b"{broken").unwrap();
            }
            let Image::Bytes { data, .. } =
                resolve(&paths, &format!("mangoimg://head/{id}")).unwrap()
            else {
                panic!();
            };
            assert_eq!(data, steve, "{id}");
        }
        for bad in ["../x", "zz", "a/b", &"a".repeat(40)] {
            assert!(
                resolve(&paths, &format!("mangoimg://head/{bad}")).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn alex_follows_java_hash_parity() {
        // hashCode = (hi ^ lo) folded to 32 bits; odd is Alex.
        assert!(!is_alex("00000000000000000000000000000000"));
        assert!(is_alex("00000000-0000-0000-0000-000000000001"));
        assert!(!is_alex("00000000-0000-0000-0000-000000000002"));
        assert!(!is_alex("not a uuid"));
    }

    #[test]
    fn remote_icons_are_cached_or_need_a_fetch() {
        let (_t, paths) = setup();
        let url = "https://cdn.modrinth.com/data/abc/icon.png";
        let Err(ImageError::NeedsFetch(r)) = resolve(&paths, &remote_url(url)) else {
            panic!("expected NeedsFetch");
        };
        assert_eq!(r.url, url);
        assert!(r.stem.starts_with(mod_icons_dir(&paths)));
        touch(&r.stem.with_extension("png"));
        assert_eq!(
            ok_path(resolve(&paths, &remote_url(url))),
            std::fs::canonicalize(r.stem.with_extension("png")).unwrap()
        );
        for bad in [
            "ftp://x/y.png",
            "https://user:pw@cdn.modrinth.com/a.png",
            "javascript:alert(1)",
            "not a url",
        ] {
            assert_eq!(
                resolve(&paths, &remote_url(bad)).unwrap_err(),
                ImageError::BadUrl,
                "{bad}"
            );
        }
    }

    #[test]
    fn sniffing_and_mime() {
        assert_eq!(sniff_extension(b"\x89PNG\r\n\x1a\nxx"), Some("png"));
        assert_eq!(sniff_extension(b"\xff\xd8\xff\xe0"), Some("jpg"));
        assert_eq!(sniff_extension(b"GIF89a.."), Some("gif"));
        assert_eq!(sniff_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_extension(b"<svg/>"), None);
        assert_eq!(mime_for(Path::new("a.SVG")), None);
        assert_eq!(ImageError::Forbidden.status(), 403);
        assert_eq!(ImageError::NotFound.status(), 404);
        assert_eq!(ImageError::BadUrl.status(), 400);
    }
}

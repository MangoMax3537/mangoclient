//! Block icon gallery: real block faces extracted at runtime from the user's
//! own Minecraft client jar (nothing is bundled).
//!
//! Layout: `cache/block-icons/current.json` names the active version,
//! `cache/block-icons/<ver>/ready.json` lists the finished blocks and
//! `cache/block-icons/<ver>/<id>/{top,left,right}.png` are the faces. Only
//! the whitelisted texture entries of the jar are ever read.

use crate::error::{AppError, Res};
use crate::events::Event;
use crate::icon_images::{decode_image, encode_png, write_atomic};
use crate::service::{lock, AppService};
use image::{ImageFormat, RgbaImage};
use mango_core::Paths;
use mango_minecraft::{get_version_manifest, validate_safe_name};
use mango_net::{download_file, get_json, DownloadOptions, Expected};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Grass colour of the plains biome.
pub const PLAINS_TINT: [u8; 3] = [0x91, 0xBD, 0x59];
/// Per-texture size cap.
pub const MAX_TEXTURE_BYTES: u64 = 1024 * 1024;
/// A gallery with fewer blocks than this counts as failed.
const MIN_BLOCKS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Src {
    Plain(&'static str),
    Tinted(&'static str),
    /// `grass_block_side` + tinted `grass_block_side_overlay`.
    GrassSide,
}

pub(crate) struct BlockDef {
    pub id: &'static str,
    pub label: &'static str,
    pub top: Src,
    pub left: Src,
    pub right: Src,
}

const fn uni(id: &'static str, label: &'static str, tex: &'static str) -> BlockDef {
    BlockDef {
        id,
        label,
        top: Src::Plain(tex),
        left: Src::Plain(tex),
        right: Src::Plain(tex),
    }
}

const fn sided(
    id: &'static str,
    label: &'static str,
    top: &'static str,
    side: &'static str,
) -> BlockDef {
    BlockDef {
        id,
        label,
        top: Src::Plain(top),
        left: Src::Plain(side),
        right: Src::Plain(side),
    }
}

pub(crate) const BLOCKS: &[BlockDef] = &[
    BlockDef {
        id: "grass_block",
        label: "Grass Block",
        top: Src::Tinted("grass_block_top"),
        left: Src::GrassSide,
        right: Src::GrassSide,
    },
    uni("dirt", "Dirt", "dirt"),
    uni("stone", "Stone", "stone"),
    uni("cobblestone", "Cobblestone", "cobblestone"),
    uni("oak_planks", "Oak Planks", "oak_planks"),
    sided("oak_log", "Oak Log", "oak_log_top", "oak_log"),
    BlockDef {
        id: "crafting_table",
        label: "Crafting Table",
        top: Src::Plain("crafting_table_top"),
        left: Src::Plain("crafting_table_side"),
        right: Src::Plain("crafting_table_front"),
    },
    BlockDef {
        id: "furnace",
        label: "Furnace",
        top: Src::Plain("furnace_top"),
        left: Src::Plain("furnace_side"),
        right: Src::Plain("furnace_front"),
    },
    sided("tnt", "TNT", "tnt_top", "tnt_side"),
    uni("bricks", "Bricks", "bricks"),
    sided("bookshelf", "Bookshelf", "oak_planks", "bookshelf"),
    uni("diamond_ore", "Diamond Ore", "diamond_ore"),
    uni("gold_ore", "Gold Ore", "gold_ore"),
    uni("iron_ore", "Iron Ore", "iron_ore"),
    uni("emerald_ore", "Emerald Ore", "emerald_ore"),
    uni("redstone_ore", "Redstone Ore", "redstone_ore"),
    uni("lapis_block", "Lapis Block", "lapis_block"),
    sided("sandstone", "Sandstone", "sandstone_top", "sandstone"),
    uni("netherrack", "Netherrack", "netherrack"),
    uni("obsidian", "Obsidian", "obsidian"),
    sided("pumpkin", "Pumpkin", "pumpkin_top", "pumpkin_side"),
];

pub(crate) const FACES: [&str; 3] = ["top", "left", "right"];

/// Reserved `iconBlock` value for the real MangoClient brand icon. Not a block, so
/// it needs no extracted gallery; old data (block ids only) is unaffected.
pub const BRAND_ICON_ID: &str = "mangoclient";

pub(crate) fn is_known_block(id: &str) -> bool {
    id == BRAND_ICON_ID || BLOCKS.iter().any(|b| b.id == id)
}

/// Every texture name the extractor may read.
#[cfg(test)]
pub(crate) fn texture_names() -> Vec<&'static str> {
    let mut out = Vec::new();
    for b in BLOCKS {
        for s in [b.top, b.left, b.right] {
            match s {
                Src::Plain(n) | Src::Tinted(n) => out.push(n),
                Src::GrassSide => {
                    out.push("grass_block_side");
                    out.push("grass_block_side_overlay");
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BlockIcon {
    pub id: String,
    pub label: String,
    #[ts(as = "crate::wire::BlockFaces")]
    pub faces: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum BlocksState {
    Missing,
    Preparing,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlockIconsStatus {
    pub state: BlocksState,
    pub mc_version: Option<String>,
    pub blocks: Vec<BlockIcon>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// Run state of the background job.
#[derive(Debug, Default)]
pub(crate) struct BlocksRun {
    pub preparing: bool,
    pub error: Option<String>,
}

pub(crate) fn blocks_root(paths: &Paths) -> PathBuf {
    paths.cache().join("block-icons")
}

/// A version id usable as a folder name in the cache.
pub(crate) fn safe_version_dir(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && !v.starts_with('.')
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}

/// `1.21.11` -> [1, 21, 11]; `None` for anything that is not a plain release.
pub(crate) fn release_key(id: &str) -> Option<Vec<u32>> {
    let parts: Vec<u32> = id
        .split('.')
        .map(|p| p.parse::<u32>().ok())
        .collect::<Option<_>>()?;
    (2..=4).contains(&parts.len()).then_some(parts)
}

pub(crate) fn face_url(id: &str, face: &str) -> String {
    format!("mangoimg://block/{id}/{face}")
}

fn read_json(path: &Path) -> Option<Value> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 64 * 1024 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The finished gallery on disk: version plus the blocks whose three faces
/// all exist, in fixed list order.
pub(crate) fn gallery(paths: &Paths) -> Option<(String, Vec<&'static BlockDef>)> {
    let root = blocks_root(paths);
    let current = read_json(&root.join("current.json"))?;
    let version = current.get("mcVersion")?.as_str()?.to_string();
    if !safe_version_dir(&version) {
        return None;
    }
    let ready = read_json(&root.join(&version).join("ready.json"))?;
    let ids: Vec<&str> = ready
        .get("blocks")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let blocks: Vec<&BlockDef> = BLOCKS
        .iter()
        .filter(|b| ids.contains(&b.id))
        .filter(|b| {
            FACES.iter().all(|f| {
                root.join(&version)
                    .join(b.id)
                    .join(format!("{f}.png"))
                    .is_file()
            })
        })
        .collect();
    (!blocks.is_empty()).then_some((version, blocks))
}

/// FNV-1a over the profile id, modulo the gallery length.
pub(crate) fn auto_block(paths: &Paths, profile_id: &str) -> Option<String> {
    let (_, blocks) = gallery(paths)?;
    let mut h: u32 = 0x811c_9dc5;
    for b in profile_id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    blocks
        .get(h as usize % blocks.len())
        .map(|b| b.id.to_string())
}

fn icon_of(b: &BlockDef) -> BlockIcon {
    BlockIcon {
        id: b.id.to_string(),
        label: b.label.to_string(),
        faces: FACES
            .iter()
            .map(|f| ((*f).to_string(), face_url(b.id, f)))
            .collect(),
    }
}

// ---- pixel work ----

/// Multiply a grey texture by a tint (rounded).
pub(crate) fn tint(img: &mut RgbaImage, t: [u8; 3]) {
    for px in img.pixels_mut() {
        for (v, k) in px.0.iter_mut().zip(t) {
            *v = ((u32::from(*v) * u32::from(k) + 127) / 255) as u8;
        }
    }
}

/// `over` composited on `base` (straight alpha, rounded).
pub(crate) fn composite(base: &RgbaImage, over: &RgbaImage) -> Res<RgbaImage> {
    if base.dimensions() != over.dimensions() {
        return Err(AppError::new("blocks.bad_texture", "Overlay size differs"));
    }
    let mut out = base.clone();
    for (o, (b, t)) in out.pixels_mut().zip(base.pixels().zip(over.pixels())) {
        let (ab, at) = (u32::from(b.0[3]), u32::from(t.0[3]));
        let a = at * 255 + ab * (255 - at); // alpha scaled by 255
        if a == 0 {
            o.0 = [0, 0, 0, 0];
            continue;
        }
        for c in 0..3 {
            let top = u32::from(t.0[c]) * at * 255;
            let bottom = u32::from(b.0[c]) * ab * (255 - at);
            o.0[c] = ((top + bottom + a / 2) / a) as u8;
        }
        o.0[3] = ((a + 127) / 255) as u8;
    }
    Ok(out)
}

/// First square frame (animated strips are taller than wide).
fn first_square(img: RgbaImage) -> RgbaImage {
    let side = img.width().min(img.height());
    if img.width() == side && img.height() == side {
        return img;
    }
    image::imageops::crop_imm(&img, 0, 0, side, side).to_image()
}

/// Read one whitelisted entry, bounded.
pub(crate) fn read_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    cap: u64,
) -> Res<Vec<u8>> {
    let entry = zip
        .by_name(name)
        .map_err(|_| AppError::new("blocks.texture_missing", format!("The jar has no {name}")))?;
    if !entry.is_file() || entry.size() > cap {
        return Err(AppError::new(
            "blocks.texture_too_large",
            format!("{name} is not an acceptable file"),
        ));
    }
    let mut buf = Vec::new();
    entry.take(cap + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > cap {
        return Err(AppError::new(
            "blocks.texture_too_large",
            "Texture too large",
        ));
    }
    Ok(buf)
}

struct Textures<'a, R: Read + Seek> {
    zip: &'a mut zip::ZipArchive<R>,
    cache: HashMap<&'static str, RgbaImage>,
}

impl<R: Read + Seek> Textures<'_, R> {
    fn get(&mut self, name: &'static str) -> Res<RgbaImage> {
        if let Some(img) = self.cache.get(name) {
            return Ok(img.clone());
        }
        let entry = format!("assets/minecraft/textures/block/{name}.png");
        let bytes = read_entry(self.zip, &entry, MAX_TEXTURE_BYTES)?;
        let img = first_square(decode_image(&bytes, Some(ImageFormat::Png))?.to_rgba8());
        self.cache.insert(name, img.clone());
        Ok(img)
    }

    fn face(&mut self, src: Src) -> Res<RgbaImage> {
        match src {
            Src::Plain(n) => self.get(n),
            Src::Tinted(n) => {
                let mut img = self.get(n)?;
                tint(&mut img, PLAINS_TINT);
                Ok(img)
            }
            Src::GrassSide => {
                let base = self.get("grass_block_side")?;
                let mut over = self.get("grass_block_side_overlay")?;
                tint(&mut over, PLAINS_TINT);
                composite(&base, &over)
            }
        }
    }
}

/// Extract the gallery of `jar` (version `version`) into `root`
/// (`cache/block-icons`). Returns the finished block ids. Without `force`
/// a complete earlier run is reused.
pub(crate) fn extract_blocks(
    jar: &Path,
    root: &Path,
    version: &str,
    force: bool,
) -> Res<Vec<&'static str>> {
    if !safe_version_dir(version) {
        return Err(AppError::new("blocks.bad_version", "Unsafe version id"));
    }
    let dir = root.join(version);
    if !force {
        if let Some(done) = complete_run(root, version) {
            write_current(root, version)?;
            return Ok(done);
        }
    }
    let file = std::fs::File::open(jar)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| AppError::new("blocks.jar_unreadable", e.to_string()))?;
    let mut tex = Textures {
        zip: &mut zip,
        cache: HashMap::new(),
    };
    let mut done = Vec::new();
    for b in BLOCKS {
        let faces: Res<Vec<RgbaImage>> = [b.top, b.left, b.right]
            .into_iter()
            .map(|s| tex.face(s))
            .collect();
        let faces = match faces {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!("block {} skipped: {e}", b.id);
                continue;
            }
        };
        for (name, img) in FACES.iter().zip(&faces) {
            write_atomic(
                &dir.join(b.id).join(format!("{name}.png")),
                &encode_png(img)?,
            )?;
        }
        done.push(b.id);
    }
    if done.len() < MIN_BLOCKS {
        return Err(AppError::new(
            "blocks.incomplete",
            "The Minecraft jar has too few of the expected block textures",
        ));
    }
    let ready = serde_json::to_vec(&serde_json::json!({"blocks": done}))?;
    write_atomic(&dir.join("ready.json"), &ready)?;
    write_current(root, version)?;
    Ok(done)
}

fn write_current(root: &Path, version: &str) -> Res<()> {
    let cur = serde_json::to_vec(&serde_json::json!({"mcVersion": version}))?;
    write_atomic(&root.join("current.json"), &cur)?;
    Ok(())
}

/// The block ids of an earlier complete run of `version`, if intact.
fn complete_run(root: &Path, version: &str) -> Option<Vec<&'static str>> {
    let dir = root.join(version);
    let ready = read_json(&dir.join("ready.json"))?;
    let ids: Vec<&str> = ready
        .get("blocks")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let blocks: Vec<&'static str> = BLOCKS
        .iter()
        .filter(|b| ids.contains(&b.id))
        .map(|b| b.id)
        .collect();
    let intact = blocks.len() >= MIN_BLOCKS
        && blocks.iter().all(|id| {
            FACES
                .iter()
                .all(|f| dir.join(id).join(format!("{f}.png")).is_file())
        });
    intact.then_some(blocks)
}

// ---- jar selection ----

/// `(key, id, jar)` of installed release jars, newest first.
fn release_jars_in(dir: &Path, nested: bool) -> Vec<(Vec<u32>, String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let (id, jar) = if nested {
            (name.clone(), e.path().join(format!("{name}.jar")))
        } else if let Some(id) = name.strip_suffix(".jar") {
            (id.to_string(), e.path())
        } else {
            continue;
        };
        let Some(key) = release_key(&id) else {
            continue;
        };
        if validate_safe_name(&id).is_ok() && std::fs::metadata(&jar).is_ok_and(|m| m.is_file()) {
            out.push((key, id, jar));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

/// The jar of an installed version, whatever its id.
pub(crate) fn installed_jar(paths: &Paths, version: &str) -> Option<PathBuf> {
    validate_safe_name(version).ok()?;
    let jar = paths
        .versions()
        .join(version)
        .join(format!("{version}.jar"));
    std::fs::metadata(&jar).ok()?.is_file().then_some(jar)
}

/// Newest installed release, else the newest jar this feature downloaded.
pub(crate) fn local_jar(paths: &Paths) -> Option<(String, PathBuf)> {
    let installed = release_jars_in(&paths.versions(), true);
    let cached = release_jars_in(&blocks_root(paths).join("_jar"), false);
    installed
        .into_iter()
        .chain(cached)
        .next()
        .map(|(_, id, jar)| (id, jar))
}

impl AppService {
    /// A local jar, else the latest release's client jar from Mojang
    /// (sha1 + size verified; no libraries, no assets).
    pub(crate) async fn block_source_jar(&self) -> Res<(String, PathBuf)> {
        if let Some(found) = local_jar(&self.inner.paths) {
            return Ok(found);
        }
        let opts = &self.inner.opts;
        let manifest =
            get_version_manifest(&self.inner.paths, &opts.manifest_url, false, &opts.fetch).await?;
        let bad = || AppError::new("blocks.bad_manifest", "Unexpected Mojang manifest");
        let latest = manifest["latest"]["release"].as_str().ok_or_else(bad)?;
        validate_safe_name(latest)?;
        let meta_url = manifest["versions"]
            .as_array()
            .and_then(|vs| vs.iter().find(|v| v["id"].as_str() == Some(latest)))
            .and_then(|v| v["url"].as_str())
            .ok_or_else(bad)?;
        let meta: Value = get_json(meta_url, &opts.fetch).await?;
        let client = &meta["downloads"]["client"];
        let (url, sha1, size) = (
            client["url"].as_str().ok_or_else(bad)?,
            client["sha1"].as_str().ok_or_else(bad)?,
            client["size"].as_u64().ok_or_else(bad)?,
        );
        let dest = blocks_root(&self.inner.paths)
            .join("_jar")
            .join(format!("{latest}.jar"));
        download_file(
            url,
            &dest,
            &DownloadOptions {
                expected: Expected {
                    sha1: Some(sha1.to_string()),
                    size: Some(size),
                    ..Expected::default()
                },
                fetch: opts.fetch.clone(),
                ..DownloadOptions::default()
            },
        )
        .await?;
        Ok((latest.to_string(), dest))
    }

    pub fn block_icons_status(&self) -> BlockIconsStatus {
        let (preparing, error) = {
            let run = lock(&self.inner.blocks_run);
            (run.preparing, run.error.clone())
        };
        let found = gallery(&self.inner.paths);
        let (mc_version, blocks) = match &found {
            Some((v, b)) => (Some(v.clone()), b.iter().map(|b| icon_of(b)).collect()),
            None => (None, Vec::new()),
        };
        let state = if preparing {
            BlocksState::Preparing
        } else if error.is_some() {
            BlocksState::Error
        } else if found.is_some() {
            BlocksState::Ready
        } else {
            BlocksState::Missing
        };
        BlockIconsStatus {
            state,
            mc_version,
            blocks,
            error_code: if preparing { None } else { error },
        }
    }

    fn emit_blocks_status(&self) {
        self.inner
            .emit(Event::BlocksStatus(self.block_icons_status()));
    }

    /// Start the background extraction (returns `preparing` at once).
    pub async fn block_icons_prepare(&self, force: bool) -> Res<BlockIconsStatus> {
        {
            let mut run = lock(&self.inner.blocks_run);
            if run.preparing {
                drop(run);
                return Ok(self.block_icons_status());
            }
            run.preparing = true;
            run.error = None;
        }
        let status = self.block_icons_status();
        self.inner.emit(Event::BlocksStatus(status.clone()));
        let svc = self.clone();
        tokio::spawn(async move {
            let result = svc.run_block_prepare(force).await;
            {
                let mut run = lock(&svc.inner.blocks_run);
                run.preparing = false;
                run.error = result.err().map(|e| {
                    tracing::warn!("block icons: {e}");
                    e.code
                });
            }
            svc.emit_blocks_status();
        });
        Ok(status)
    }

    async fn run_block_prepare(&self, force: bool) -> Res<()> {
        let (version, jar) = self.block_source_jar().await?;
        let root = blocks_root(&self.inner.paths);
        let (v, j) = (version.clone(), jar.clone());
        tokio::task::spawn_blocking(move || extract_blocks(&j, &root, &v, force)).await??;
        // The same jar also carries the title-screen panorama.
        let paths = self.inner.paths.clone();
        let _ = tokio::task::spawn_blocking(move || {
            crate::backgrounds::extract_panoramas(&jar, &paths, &version, force)
        })
        .await;
        Ok(())
    }
}

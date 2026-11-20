//! Jar metadata: Fabric, Quilt, Forge/NeoForge TOML, legacy mcmod.info.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use regex::Regex;
use serde_json::Value;
use sha1::{Digest, Sha1};
use zip::ZipArchive;

/// A logo big enough to matter is not worth caching.
const MAX_ICON_BYTES: u64 = 256 * 1024;
/// Metadata text files larger than this are ignored.
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JarMeta {
    pub title: String,
    pub version: String,
    /// `mangoimg://f/<base64url of abs path>` of the cached icon.
    pub icon: Option<String>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> Option<&'static Regex> {
    if let Some(r) = cell.get() {
        return Some(r);
    }
    let r = Regex::new(pat).ok()?;
    Some(cell.get_or_init(|| r))
}

pub(crate) fn image_url(file: &Path) -> String {
    format!(
        "mangoimg://f/{}",
        URL_SAFE_NO_PAD.encode(file.to_string_lossy().as_bytes())
    )
}

fn strip_disabled(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".disabled") {
        &name[..name.len() - ".disabled".len()]
    } else {
        name
    }
}

fn base_name(filename: &str) -> String {
    let n = strip_disabled(filename);
    let lower = n.to_ascii_lowercase();
    if lower.ends_with(".jar") {
        n[..n.len() - 4].to_string()
    } else {
        n.to_string()
    }
}

/// Split `name-1.2.3` into (`name`, `1.2.3`), as `[-_]v?\d[\w.+]*$`.
fn split_version(base: &str) -> (String, Option<String>) {
    static R: OnceLock<Regex> = OnceLock::new();
    let Some(r) = re(&R, r"[-_](v?\d[\w.+]*)$") else {
        return (base.to_string(), None);
    };
    match r.captures(base) {
        Some(c) => {
            let whole = c.get(0).map(|m| m.start()).unwrap_or(base.len());
            let ver = c.get(1).map(|m| m.as_str().to_string());
            (base[..whole].to_string(), ver)
        }
        None => (base.to_string(), None),
    }
}

fn fallback(filename: &str) -> JarMeta {
    let base = base_name(filename);
    let (stem, version) = split_version(&base);
    static R: OnceLock<Regex> = OnceLock::new();
    let spaced = match re(&R, r"[-_]+") {
        Some(r) => r.replace_all(&stem, " ").trim().to_string(),
        None => stem,
    };
    JarMeta {
        title: if spaced.is_empty() { base } else { spaced },
        version: version.unwrap_or_default(),
        icon: None,
    }
}

type Zip = ZipArchive<File>;

fn read_entry(zip: &mut Zip, name: &str, limit: u64) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name.trim_start_matches('/')).ok()?;
    if entry.is_dir() || entry.size() > limit {
        return None;
    }
    let mut buf = Vec::new();
    entry.by_ref().take(limit).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn read_text(zip: &mut Zip, name: &str) -> Option<String> {
    let bytes = read_entry(zip, name, MAX_METADATA_BYTES)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(text.trim_start_matches('\u{feff}').to_string())
}

fn icon_url(zip: &mut Zip, logo: Option<&str>, icon_dir: &Path) -> Option<String> {
    let logo = logo?;
    if !logo.to_ascii_lowercase().ends_with(".png") {
        return None;
    }
    let data = read_entry(zip, logo, MAX_ICON_BYTES)?;
    let hash = hex::encode(Sha1::digest(&data));
    let file = icon_dir.join(format!("{hash}.png"));
    std::fs::create_dir_all(icon_dir).ok()?;
    if !file.exists() {
        std::fs::write(&file, &data).ok()?;
    }
    Some(image_url(&file))
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `icon` is either a path or a map of size -> path; take the 64px one.
fn icon_path(v: &Value) -> Option<String> {
    match v.get("icon")? {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => m.get("64").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

struct Toml {
    title: Option<String>,
    version: Option<String>,
    logo: Option<String>,
}

fn toml_field(block: &str, key: &str) -> Option<String> {
    let multi = Regex::new(&format!(r"(?m)^\s*{key}\s*=\s*'''([\s\S]*?)'''")).ok()?;
    let single = Regex::new(&format!(r#"(?m)^\s*{key}\s*=\s*["'](.*?)["']"#)).ok()?;
    single
        .captures(block)
        .or_else(|| multi.captures(block))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn parse_mods_toml(text: &str) -> Option<Toml> {
    const TAG: &str = "[[mods]]";
    let start = text.find(TAG)?;
    let rest = &text[start + TAG.len()..];
    let block = rest.find("[[").map_or(rest, |e| &rest[..e]);
    Some(Toml {
        title: toml_field(block, "displayName").or_else(|| toml_field(block, "modId")),
        version: toml_field(block, "version"),
        logo: toml_field(text, "logoFile"),
    })
}

/// Forge writes `${file.jarVersion}`, which only the manifest can resolve.
fn manifest_version(zip: &mut Zip) -> Option<String> {
    let mf = read_text(zip, "META-INF/MANIFEST.MF")?;
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, r"(?m)^Implementation-Version:[ \t]*(.+)$")?;
    r.captures(&mf)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn parse_json_lenient(text: &str) -> Option<Value> {
    static R: OnceLock<Regex> = OnceLock::new();
    let cleaned = match re(&R, r",(\s*[}\]])") {
        Some(r) => r.replace_all(text, "$1").into_owned(),
        None => text.to_string(),
    };
    serde_json::from_str(&cleaned).ok()
}

/// Read a jar's own name, version and logo; falls back to the file name.
/// Icons are written to `icon_dir` as `<sha1>.png`.
pub fn read_jar_meta(file: &Path, icon_dir: &Path) -> JarMeta {
    let filename = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fb = fallback(&filename);
    let Ok(handle) = File::open(file) else {
        return fb;
    };
    let Ok(mut zip) = ZipArchive::new(handle) else {
        return fb;
    };
    from_zip(&mut zip, &fb, icon_dir).unwrap_or(fb)
}

fn from_zip(zip: &mut Zip, fb: &JarMeta, icon_dir: &Path) -> Option<JarMeta> {
    if let Some(text) = read_text(zip, "fabric.mod.json") {
        // Malformed metadata: the file name still tells us enough.
        let data = parse_json_lenient(&text)?;
        let icon = icon_path(&data);
        return Some(JarMeta {
            title: str_field(&data, "name")
                .or_else(|| str_field(&data, "id"))
                .unwrap_or_else(|| fb.title.clone()),
            version: str_field(&data, "version").unwrap_or_else(|| fb.version.clone()),
            icon: icon_url(zip, icon.as_deref(), icon_dir),
        });
    }
    if let Some(text) = read_text(zip, "quilt.mod.json") {
        let data: Value = serde_json::from_str(&text).ok()?;
        let loader = data.get("quilt_loader").cloned().unwrap_or(Value::Null);
        let meta = loader.get("metadata").cloned().unwrap_or(Value::Null);
        let icon = icon_path(&meta);
        return Some(JarMeta {
            title: str_field(&meta, "name")
                .or_else(|| str_field(&loader, "id"))
                .unwrap_or_else(|| fb.title.clone()),
            version: str_field(&loader, "version").unwrap_or_else(|| fb.version.clone()),
            icon: icon_url(zip, icon.as_deref(), icon_dir),
        });
    }
    let toml = read_text(zip, "META-INF/neoforge.mods.toml")
        .or_else(|| read_text(zip, "META-INF/mods.toml"));
    if let Some(text) = toml {
        let data = parse_mods_toml(&text).unwrap_or(Toml {
            title: None,
            version: None,
            logo: None,
        });
        let mut version = data.version.filter(|v| !v.is_empty());
        let unresolved = match &version {
            None => true,
            Some(v) => v.contains("${"),
        };
        if unresolved {
            version = manifest_version(zip).or_else(|| Some(fb.version.clone()));
        }
        return Some(JarMeta {
            title: data.title.unwrap_or_else(|| fb.title.clone()),
            version: version.unwrap_or_default(),
            icon: icon_url(zip, data.logo.as_deref(), icon_dir),
        });
    }
    if let Some(text) = read_text(zip, "mcmod.info") {
        let list: Value = serde_json::from_str(&text).ok()?;
        let first = match &list {
            Value::Array(a) => a.first(),
            other => other.get("modList").and_then(|m| m.get(0)),
        };
        if let Some(first) = first {
            return Some(JarMeta {
                title: str_field(first, "name")
                    .or_else(|| str_field(first, "modid"))
                    .unwrap_or_else(|| fb.title.clone()),
                version: str_field(first, "version").unwrap_or_else(|| fb.version.clone()),
                icon: None,
            });
        }
    }
    None
}

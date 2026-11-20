//! Typed, bounded validation of config and profile patches (JSON in, cleaned
//! JSON out). Mirrors `src/main/validation.js`.

use serde_json::{json, Map, Value};

use crate::model::ModRecord;
use crate::paths::is_canonical_uuid;

pub const LOADERS: [&str; 5] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];
const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("{0} must be an object")]
    NotObject(String),
    #[error("Invalid {0}")]
    Invalid(String),
    #[error("Unknown {scope} setting: {key}")]
    Unknown { scope: String, key: String },
    #[error("Profile IDs are immutable")]
    ImmutableId,
}

type R<T> = std::result::Result<T, ValidationError>;

fn invalid(name: &str) -> ValidationError {
    ValidationError::Invalid(name.to_string())
}

/// A bundled wallpaper's file stem: `[a-z0-9-]`, 1 to 64 long.
pub fn is_wallpaper_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// 32 hex digits or canonical UUID.
pub fn is_uuid(s: &str) -> bool {
    (s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit())) || is_canonical_uuid(s)
}

/// `^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$`
pub fn is_safe_version(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphanumeric()
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'))
}

/// `^#[0-9a-f]{6}$` (case-insensitive)
pub fn is_color(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

fn object<'a>(v: &'a Value, label: &str) -> R<&'a Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| ValidationError::NotObject(label.to_string()))
}

fn boolean(v: &Value, name: &str) -> R<Value> {
    v.as_bool().map(Value::Bool).ok_or_else(|| invalid(name))
}

fn text_str<'a>(v: &'a Value, name: &str, max: usize, allow_empty: bool) -> R<&'a str> {
    let s = v.as_str().ok_or_else(|| invalid(name))?;
    // JS length counts UTF-16 units.
    if s.encode_utf16().count() > max || (!allow_empty && s.trim().is_empty()) || s.contains('\0') {
        return Err(invalid(name));
    }
    Ok(s)
}

fn text(v: &Value, name: &str, max: usize, allow_empty: bool) -> R<Value> {
    text_str(v, name, max, allow_empty).map(|s| Value::String(s.to_string()))
}

fn int_val(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    if let Some(u) = v.as_u64() {
        return i64::try_from(u).ok();
    }
    match v.as_f64() {
        Some(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE_INT as f64 => Some(f as i64),
        _ => None,
    }
}

fn integer(v: &Value, name: &str, min: i64, max: i64) -> R<Value> {
    match int_val(v) {
        Some(i) if i >= min && i <= max => Ok(json!(i)),
        _ => Err(invalid(name)),
    }
}

/// `null` or a finite number in `0..=max`; out of range is an error, never clamped.
fn nullable_range(v: &Value, name: &str, max: f64) -> R<Value> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && (0.0..=max).contains(&f) => Ok(json!(f)),
            _ => Err(invalid(name)),
        },
        _ => Err(invalid(name)),
    }
}

fn nullable_id(v: &Value, name: &str) -> R<Value> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::String(s) if is_uuid(s) => Ok(v.clone()),
        _ => Err(invalid(name)),
    }
}

fn enum_value(v: &Value, name: &str, values: &[&str]) -> R<Value> {
    match v.as_str() {
        Some(s) if values.contains(&s) => Ok(v.clone()),
        _ => Err(invalid(name)),
    }
}

/// Validate a single config key; `None` when the key is unknown.
pub fn config_field(key: &str, v: &Value) -> Option<R<Value>> {
    Some(match key {
        "language" => enum_value(v, "language", &["de", "en"]),
        "theme" => enum_value(v, "theme", &["mango"]),
        "ram" => integer(v, "ram", 1024, 131072),
        "javaPath" => text(v, "javaPath", 4096, true),
        "javaArgs" => text(v, "javaArgs", 8192, true),
        "fullscreen" | "keepLauncherOpen" | "hideOnLaunch" | "showSnapshots" | "mangoConfig"
        | "performanceMods" | "sidebarOpen" | "firstRunDone" | "telemetry" => boolean(v, key),
        "width" => integer(v, "width", 640, 16384),
        "height" => integer(v, "height", 480, 16384),
        "concurrentDownloads" => integer(v, "concurrentDownloads", 1, 32),
        "performancePreset" => {
            enum_value(v, "performancePreset", &["potato", "balanced", "quality"])
        }
        "selectedProfile" | "selectedAccount" => nullable_id(v, key),
        "autoSnapshot" => boolean(v, key),
        "backupKeep" => integer(v, "backupKeep", 1, 50),
        _ => return None,
    })
}

fn filter_patch(
    patch: &Value,
    label: &str,
    field: impl Fn(&str, &Value) -> Option<R<Value>>,
) -> R<Map<String, Value>> {
    let obj = object(patch, label)?;
    let mut clean = Map::new();
    for (key, value) in obj {
        let validated = field(key, value).ok_or_else(|| ValidationError::Unknown {
            scope: label.to_string(),
            key: key.clone(),
        })??;
        clean.insert(key.clone(), validated);
    }
    Ok(clean)
}

pub fn config_patch(patch: &Value) -> R<Map<String, Value>> {
    filter_patch(patch, "configuration", config_field)
}

const MOD_STRING_KEYS: [&str; 8] = [
    "projectId",
    "slug",
    "title",
    "versionId",
    "versionNumber",
    "filename",
    "file",
    "icon",
];
const MOD_KNOWN_KEYS: [&str; 17] = [
    "projectId",
    "slug",
    "title",
    "versionId",
    "versionNumber",
    "filename",
    "file",
    "icon",
    "type",
    "enabled",
    "dependency",
    "local",
    "installedAt",
    "gameVersions",
    "loaders",
    "requiredDependencies",
    "provider",
];

/// Validate one mod record. Unknown keys are dropped.
pub fn validate_mod(v: &Value) -> R<Map<String, Value>> {
    let m = object(v, "mod")?;
    let mut clean = Map::new();
    let get = |k: &str| m.get(k).filter(|x| !x.is_null());
    for key in MOD_STRING_KEYS {
        if let Some(x) = get(key) {
            let max = if key == "file" || key == "icon" {
                8192
            } else {
                512
            };
            clean.insert(key.into(), text(x, key, max, true)?);
        }
    }
    if let Some(x) = get("type") {
        clean.insert(
            "type".into(),
            enum_value(
                x,
                "mod type",
                &["mod", "shader", "resourcepack", "datapack"],
            )?,
        );
    }
    if let Some(x) = get("provider") {
        clean.insert(
            "provider".into(),
            // Legacy field: any short string is carried along unchanged.
            text(x, "provider", 32, true)?,
        );
    }
    for key in ["enabled", "dependency", "local"] {
        if let Some(x) = get(key) {
            clean.insert(key.into(), boolean(x, key)?);
        }
    }
    if let Some(x) = get("installedAt") {
        clean.insert(
            "installedAt".into(),
            integer(x, "installedAt", 0, MAX_SAFE_INT)?,
        );
    }
    for (key, cap, item, max) in [
        ("gameVersions", 128, "gameVersion", 64),
        ("loaders", 16, "loader", 32),
    ] {
        if let Some(Value::Array(a)) = m.get(key) {
            let items: R<Vec<Value>> = a
                .iter()
                .take(cap)
                .map(|x| text(x, item, max, true))
                .collect();
            clean.insert(key.into(), Value::Array(items?));
        }
    }
    if let Some(Value::Array(a)) = m.get("requiredDependencies") {
        let mut deps = Vec::new();
        for d in a.iter().take(128) {
            let o = object(d, "mod dependency")?;
            let pid = o.get("projectId").unwrap_or(&Value::Null);
            let vid = match o.get("versionId") {
                None | Some(Value::Null) => Value::Null,
                Some(x) => text(x, "dependency versionId", 512, false)?,
            };
            deps.push(json!({
                "projectId": text(pid, "dependency projectId", 512, false)?,
                "versionId": vid,
            }));
        }
        clean.insert("requiredDependencies".into(), Value::Array(deps));
    }
    Ok(clean)
}

/// Keys of a raw record that validation does not know (kept on load).
pub(crate) fn unknown_mod_keys(v: &Value) -> Map<String, Value> {
    let mut extra = Map::new();
    if let Some(m) = v.as_object() {
        for (k, x) in m {
            if !MOD_KNOWN_KEYS.contains(&k.as_str()) {
                extra.insert(k.clone(), x.clone());
            }
        }
    }
    extra
}

pub fn validate_mods(v: &Value) -> R<Vec<ModRecord>> {
    let arr = match v.as_array() {
        Some(a) if a.len() <= 10000 => a,
        _ => return Err(invalid("mods")),
    };
    arr.iter()
        .map(|m| {
            let clean = validate_mod(m)?;
            serde_json::from_value(Value::Object(clean)).map_err(|_| invalid("mod"))
        })
        .collect()
}

pub fn profile_field(key: &str, v: &Value) -> Option<R<Value>> {
    Some(match key {
        "name" => text_str(v, "profile name", 128, false).map(|s| json!(s.trim())),
        "mcVersion" => text_str(v, "Minecraft version", 64, false).and_then(|s| {
            if is_safe_version(s) {
                Ok(json!(s))
            } else {
                Err(invalid("Minecraft version"))
            }
        }),
        "loader" => enum_value(v, "loader", &LOADERS),
        "loaderVersion" => text_str(v, "loader version", 64, true).and_then(|s| {
            if s.is_empty() || is_safe_version(s) {
                Ok(json!(s))
            } else {
                Err(invalid("loader version"))
            }
        }),
        "color" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s) if is_color(s) => Ok(json!(s.to_ascii_lowercase())),
            _ => Err(invalid("color")),
        },
        "cover" => boolean(v, "cover"),
        "iconBlock" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s)
                if !s.is_empty()
                    && s.len() <= 32
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') =>
            {
                Ok(v.clone())
            }
            _ => Err(invalid("iconBlock")),
        },
        "background" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s)
                if s == "none"
                    || s == "image"
                    || s == "panorama"
                    || s.strip_prefix("wallpaper:").is_some_and(is_wallpaper_id) =>
            {
                Ok(v.clone())
            }
            _ => Err(invalid("background")),
        },
        "iconSource" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s) if s == "custom" || s == "modpack" => Ok(v.clone()),
            _ => Err(invalid("iconSource")),
        },
        "backgroundDim" => nullable_range(v, "backgroundDim", 1.0),
        "backgroundBlur" => nullable_range(v, "backgroundBlur", 24.0),
        "ram" => match v {
            Value::Null => Ok(Value::Null),
            _ => integer(v, "profile ram", 1024, 131072),
        },
        "ramSource" => match v {
            Value::Null => Ok(Value::Null),
            Value::String(s) if s == "modpack" || s == "user" => Ok(v.clone()),
            _ => Err(invalid("ramSource")),
        },
        "ramHint" => ram_hint(v),
        "javaArgs" => text(v, "profile javaArgs", 8192, true),
        "mods" => {
            validate_mods(v).and_then(|m| serde_json::to_value(m).map_err(|_| invalid("mods")))
        }
        "mangoConfig" => match v {
            Value::Null => Ok(Value::Null),
            _ => boolean(v, "mangoConfig"),
        },
        "group" => group_name(v),
        "tags" => tag_list(v),
        "session" => session(v),
        "lastPlayed" => match v {
            Value::Null => Ok(Value::Null),
            _ => integer(v, "lastPlayed", 0, MAX_SAFE_INT),
        },
        "playTimeMs" => integer(v, "playTimeMs", 0, MAX_SAFE_INT),
        _ => return None,
    })
}

/// `null` or a hint whose numbers are all in range (re-sanitised).
fn ram_hint(v: &Value) -> R<Value> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let hint: crate::ram::RamHint =
        serde_json::from_value(v.clone()).map_err(|_| invalid("ramHint"))?;
    let clean =
        crate::ram::RamHint::new(hint.recommended_mb, hint.min_mb, hint.max_mb, hint.source)
            .ok_or_else(|| invalid("ramHint"))?;
    serde_json::to_value(clean).map_err(|_| invalid("ramHint"))
}

pub const MAX_GROUP_CHARS: usize = 48;
pub const MAX_TAGS: usize = 12;
pub const MAX_TAG_CHARS: usize = 24;

/// A label for a group or tag: trimmed, no control characters (this also
/// rules out newlines and NUL), not empty.
fn label(s: &str, max: usize, name: &str) -> R<String> {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > max || t.chars().any(char::is_control) {
        return Err(invalid(name));
    }
    Ok(t.to_string())
}

/// `null` or an empty/blank string = no group; else a trimmed label of at
/// most [`MAX_GROUP_CHARS`] characters.
fn group_name(v: &Value) -> R<Value> {
    match v {
        Value::Null => Ok(Value::Null),
        Value::String(s) if s.trim().is_empty() => Ok(Value::Null),
        Value::String(s) => label(s, MAX_GROUP_CHARS, "group").map(Value::String),
        _ => Err(invalid("group")),
    }
}

/// At most [`MAX_TAGS`] tags of at most [`MAX_TAG_CHARS`] characters each,
/// trimmed. Duplicates (case-insensitive) are dropped, the first spelling
/// wins. Any bad tag rejects the whole list; so does a list that is too long
/// before de-duplication.
fn tag_list(v: &Value) -> R<Value> {
    let list = v.as_array().ok_or_else(|| invalid("tags"))?;
    if list.len() > MAX_TAGS {
        return Err(invalid("tags"));
    }
    let mut out: Vec<String> = Vec::new();
    for item in list {
        let s = item.as_str().ok_or_else(|| invalid("tags"))?;
        let t = label(s, MAX_TAG_CHARS, "tags")?;
        if !out.iter().any(|o| o.to_lowercase() == t.to_lowercase()) {
            out.push(t);
        }
    }
    Ok(json!(out))
}

fn session(v: &Value) -> R<Value> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let o = object(v, "session")?;
    let or = |k: &str, d: Value| match o.get(k) {
        None | Some(Value::Null) => d,
        Some(x) => x.clone(),
    };
    Ok(json!({
        "pid": integer(o.get("pid").unwrap_or(&Value::Null), "pid", 1, 0x7fff_ffff)?,
        "versionId": text(&or("versionId", json!("")), "versionId", 128, true)?,
        "startedAt": integer(&or("startedAt", json!(0)), "startedAt", 0, MAX_SAFE_INT)?,
    }))
}

pub fn profile_patch(patch: &Value) -> R<Map<String, Value>> {
    if object(patch, "profile")?.contains_key("id") {
        return Err(ValidationError::ImmutableId);
    }
    filter_patch(patch, "profile", profile_field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg<T: std::fmt::Debug>(r: R<T>) -> String {
        r.unwrap_err().to_string()
    }

    #[test]
    fn configuration_uses_typed_fields_bounds_and_enums() {
        let ok = config_patch(&json!({"language": "en", "ram": 4096, "fullscreen": true})).unwrap();
        assert_eq!(
            Value::Object(ok),
            json!({"language": "en", "ram": 4096, "fullscreen": true})
        );
        assert!(msg(config_patch(&json!({"surprise": true}))).contains("Unknown"));
        assert!(msg(config_patch(&json!({"concurrentDownloads": 1000}))).contains("Invalid"));
        assert!(msg(config_patch(&json!({"performancePreset": "fastest"}))).contains("Invalid"));
        assert!(config_patch(&json!([1])).is_err());
        assert!(config_patch(&json!({"ram": 4096.5})).is_err());
        assert!(config_patch(&json!({"selectedProfile": "nope"})).is_err());
        assert!(config_patch(&json!({"selectedProfile": null})).is_ok());
        assert!(config_patch(&json!({"javaPath": "a\0b"})).is_err());
    }

    #[test]
    fn profile_validation_rejects_mutable_ids_unsafe_versions_colors_and_loaders() {
        let ok = profile_patch(
            &json!({"name": "Safe", "mcVersion": "1.21.11", "loader": "fabric", "color": "#AABBCC"}),
        )
        .unwrap();
        assert_eq!(
            Value::Object(ok),
            json!({"name": "Safe", "mcVersion": "1.21.11", "loader": "fabric", "color": "#aabbcc"})
        );
        assert!(msg(profile_patch(&json!({"id": "replace-me"}))).contains("immutable"));
        assert!(msg(profile_patch(&json!({"mcVersion": "../../etc"}))).contains("Invalid"));
        assert!(msg(profile_patch(&json!({"color": "url(evil)"}))).contains("Invalid"));
        assert!(msg(profile_patch(&json!({"loader": "custom"}))).contains("Invalid"));
        assert!(profile_patch(&json!({"name": "   "})).is_err());
        assert!(profile_patch(&json!({"name": "x".repeat(129)})).is_err());
        assert!(profile_patch(&json!({"loaderVersion": "../x"})).is_err());
        assert!(profile_patch(&json!({"loaderVersion": ""})).is_ok());
        assert!(profile_patch(&json!({"bogus": 1})).is_err());
    }

    #[test]
    fn group_and_tags_are_trimmed_bounded_and_deduplicated() {
        let ok =
            profile_patch(&json!({"group": "  Survival  ", "tags": [" a ", "A", "b", "Ä", "ä"]}))
                .unwrap();
        assert_eq!(
            Value::Object(ok),
            json!({"group": "Survival", "tags": ["a", "b", "Ä"]})
        );
        for blank in [json!(null), json!(""), json!("   ")] {
            let ok = profile_patch(&json!({ "group": blank })).unwrap();
            assert_eq!(ok["group"], Value::Null);
        }
        assert!(profile_patch(&json!({"group": "x".repeat(48)})).is_ok());
        assert!(profile_patch(&json!({"group": "x".repeat(49)})).is_err());
        assert!(profile_patch(&json!({"group": "a\nb"})).is_err());
        assert!(profile_patch(&json!({"group": "a\u{0}b"})).is_err());
        assert!(profile_patch(&json!({"group": 5})).is_err());
        assert!(profile_patch(&json!({"tags": "a"})).is_err());
        assert!(profile_patch(&json!({"tags": [1]})).is_err());
        assert!(profile_patch(&json!({"tags": [""]})).is_err());
        assert!(profile_patch(&json!({"tags": ["  "]})).is_err());
        assert!(profile_patch(&json!({"tags": ["a\tb"]})).is_err());
        assert!(profile_patch(&json!({"tags": ["x".repeat(25)]})).is_err());
        assert!(profile_patch(&json!({"tags": ["x".repeat(24)]})).is_ok());
        let twelve: Vec<String> = (0..12).map(|i| format!("t{i}")).collect();
        assert!(profile_patch(&json!({ "tags": twelve })).is_ok());
        let thirteen: Vec<String> = (0..13).map(|i| format!("t{i}")).collect();
        assert!(profile_patch(&json!({ "tags": thirteen })).is_err());
        assert_eq!(
            Value::Object(profile_patch(&json!({"tags": []})).unwrap())["tags"],
            json!([])
        );
    }

    #[test]
    fn version_regex_equivalent() {
        assert!(is_safe_version("1.21.11"));
        assert!(is_safe_version("0.16.0+build.1"));
        assert!(!is_safe_version(".hidden"));
        assert!(!is_safe_version(""));
        assert!(!is_safe_version(&"a".repeat(65)));
        assert!(is_safe_version(&"a".repeat(64)));
        assert!(!is_safe_version("a b"));
    }

    #[test]
    fn mods_are_validated() {
        let ok = validate_mods(&json!([{"title": "t", "type": "mod", "file": "/x/y.jar",
            "requiredDependencies": [{"projectId": "p"}]}]))
        .unwrap();
        assert_eq!(ok.len(), 1);
        assert!(validate_mods(&json!([{"type": "weird"}])).is_err());
        assert!(validate_mods(&json!("x")).is_err());
        assert!(validate_mods(&json!([{"requiredDependencies": [{"projectId": ""}]}])).is_err());
    }

    #[test]
    fn session_validation() {
        assert!(profile_patch(&json!({"session": {"pid": 0}})).is_err());
        assert!(profile_patch(&json!({"session": {"pid": 5}})).is_ok());
        assert!(profile_patch(&json!({"session": null})).is_ok());
    }

    #[test]
    fn removed_curseforge_and_blocked_settings_are_unknown_to_patches_and_ignored_on_load() {
        for key in [
            "curseforgeApiKey",
            "blockedModsModrinthFallback",
            "blockedModsWatchDirs",
            "blockedModsCheckSubfolders",
        ] {
            assert!(config_patch(&json!({ key: null })).is_err(), "{key}");
            assert!(config_field(key, &json!(null)).is_none(), "{key}");
        }
        let raw = json!({
            "curseforgeApiKey": "$2a$10$abcdefghijklmnopqrstuv.wxyz0123456789",
            "blockedModsWatchDirs": ["/gone/for/good"],
            "blockedModsModrinthFallback": false,
            "blockedModsCheckSubfolders": true,
            "language": "en"
        });
        let c = crate::Config::from_value(&raw, 8192);
        assert_eq!(c.language, "en");
        assert!(!serde_json::to_string(&c).unwrap().contains("abcdefgh"));
    }

    #[test]
    fn mod_provider_is_carried_along_but_never_required() {
        let m = validate_mod(&json!({"projectId": "1", "provider": "curseforge"})).unwrap();
        assert_eq!(m["provider"], json!("curseforge"));
        let m = validate_mod(&json!({"projectId": "1", "provider": "gitlab"})).unwrap();
        assert_eq!(m["provider"], json!("gitlab"));
        assert!(validate_mod(&json!({"projectId": "1", "provider": 5})).is_err());
        let mods = validate_mods(&json!([{"projectId": "1", "provider": "modrinth"}])).unwrap();
        assert_eq!(mods[0].provider.as_deref(), Some("modrinth"));
        let legacy = validate_mods(&json!([{"projectId": "1"}])).unwrap();
        assert!(legacy[0].provider.is_none());
    }
}

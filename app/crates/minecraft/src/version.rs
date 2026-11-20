//! Version manifest and version JSON resolution (`inheritsFrom` flattening).

use crate::error::{MinecraftError, Result};
use mango_core::Paths;
use mango_net::{get_json, FetchOptions};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

pub const VERSION_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";
pub const MANIFEST_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// A version id or file id used as a path component must not escape its folder.
pub fn validate_safe_name(id: &str) -> Result<&str> {
    let bad = id.is_empty()
        || id == "."
        || id == ".."
        || id.contains(['/', '\\', '\0', ':'])
        || id.len() > 255;
    if bad {
        Err(MinecraftError::InvalidVersionId(id.to_string()))
    } else {
        Ok(id)
    }
}

/// The version manifest, from `cache/version_manifest.json` when fresh (6h),
/// else the network, else (offline) the stale cache.
pub async fn get_version_manifest(
    paths: &Paths,
    url: &str,
    force: bool,
    fetch: &FetchOptions,
) -> Result<Value> {
    let cache_file = paths.cache().join("version_manifest.json");
    if !force {
        if let Some(v) = read_fresh_cache(&cache_file).await {
            return Ok(v);
        }
    }
    match get_json::<Value>(url, fetch).await {
        Ok(manifest) => {
            // Caching is a nicety for offline starts; it never fails the fetch.
            let _ = tokio::fs::create_dir_all(paths.cache()).await;
            if let Ok(bytes) = serde_json::to_vec(&manifest) {
                let _ = tokio::fs::write(&cache_file, bytes).await;
            }
            Ok(manifest)
        }
        Err(err) => {
            let stale = tokio::fs::read(&cache_file)
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            stale.ok_or(MinecraftError::Net(err))
        }
    }
}

async fn read_fresh_cache(file: &Path) -> Option<Value> {
    let meta = tokio::fs::metadata(file).await.ok()?;
    let age = meta
        .modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .unwrap_or(Duration::ZERO);
    if age >= MANIFEST_TTL {
        return None;
    }
    serde_json::from_slice(&tokio::fs::read(file).await.ok()?).ok()
}

/// Everything but the version: `group:artifact[:classifier]`. Two entries with
/// the same key are one library at two versions, which the JVM must not see
/// twice (Fabric refuses to boot with two ASMs).
pub fn library_key(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    let mut key: Vec<&str> = vec![parts.first().copied().unwrap_or("")];
    key.push(parts.get(1).copied().unwrap_or(""));
    key.extend(parts.iter().skip(3));
    key.join(":")
}

fn lib_name(lib: &Value) -> &str {
    lib.get("name").and_then(Value::as_str).unwrap_or("")
}

fn present(v: Option<&Value>) -> Option<&Value> {
    v.filter(|x| !x.is_null())
}

fn array_of(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// Merge a loader version (`child`) over its vanilla parent. Child libraries
/// win for the same library key; arguments concatenate parent then child.
pub fn merge_version_json(child: &Value, parent: &Value) -> Value {
    let empty = Map::new();
    let c = child.as_object().unwrap_or(&empty);
    let p = parent.as_object().unwrap_or(&empty);
    let mut merged = p.clone();
    for (k, v) in c {
        merged.insert(k.clone(), v.clone());
    }

    let child_libs = array_of(c.get("libraries"));
    let child_keys: HashSet<String> = child_libs
        .iter()
        .map(|l| library_key(lib_name(l)))
        .collect();
    let mut libs: Vec<Value> = child_libs.to_vec();
    libs.extend(
        array_of(p.get("libraries"))
            .iter()
            .filter(|l| !child_keys.contains(&library_key(lib_name(l))))
            .cloned(),
    );
    merged.insert("libraries".into(), Value::Array(libs));

    let args_of = |m: &Map<String, Value>, k: &str| -> Vec<Value> {
        array_of(m.get("arguments").and_then(|a| a.get(k))).to_vec()
    };
    if present(p.get("arguments")).is_some() || present(c.get("arguments")).is_some() {
        let mut game = args_of(p, "game");
        game.extend(args_of(c, "game"));
        let mut jvm = args_of(p, "jvm");
        jvm.extend(args_of(c, "jvm"));
        merged.insert("arguments".into(), json!({"game": game, "jvm": jvm}));
    }
    if present(p.get("minecraftArguments")).is_some()
        && present(c.get("minecraftArguments")).is_none()
    {
        if let Some(v) = p.get("minecraftArguments") {
            merged.insert("minecraftArguments".into(), v.clone());
        }
    }
    for key in [
        "downloads",
        "assetIndex",
        "assets",
        "javaVersion",
        "logging",
    ] {
        match present(c.get(key)).or(present(p.get(key))) {
            Some(v) => {
                merged.insert(key.into(), v.clone());
            }
            None => {
                merged.remove(key);
            }
        }
    }
    if let Some(id) = c.get("id") {
        merged.insert("id".into(), id.clone());
    }
    merged.remove("inheritsFrom");
    Value::Object(merged)
}

/// A flattened version plus the id of its direct parent, if it had one.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub version: Value,
    pub parent_id: Option<String>,
}

async fn load_version_json(
    paths: &Paths,
    id: &str,
    manifest_url: &str,
    fetch: &FetchOptions,
) -> Result<Value> {
    let file = paths.versions().join(id).join(format!("{id}.json"));
    if let Some(v) = tokio::fs::read(&file)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    {
        return Ok(v);
    }
    let manifest = get_version_manifest(paths, manifest_url, false, fetch).await?;
    let url = manifest
        .get("versions")
        .and_then(Value::as_array)
        .and_then(|vs| {
            vs.iter()
                .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
        })
        .and_then(|v| v.get("url").and_then(Value::as_str))
        .ok_or_else(|| MinecraftError::UnknownVersion(id.to_string()))?
        .to_string();
    let json: Value = get_json(&url, fetch).await?;
    tokio::fs::create_dir_all(file.parent().unwrap_or(&paths.versions())).await?;
    let pretty = serde_json::to_vec_pretty(&json).map_err(|source| MinecraftError::Json {
        what: id.to_string(),
        source,
    })?;
    tokio::fs::write(&file, pretty).await?;
    Ok(json)
}

/// Load a version JSON (downloading and caching it when absent) and flatten
/// its `inheritsFrom` chain.
pub async fn resolve_version(
    paths: &Paths,
    id: &str,
    manifest_url: &str,
    fetch: &FetchOptions,
) -> Result<ResolvedVersion> {
    let mut seen = HashSet::new();
    let mut chain: Vec<Value> = Vec::new();
    let mut current = id.to_string();
    loop {
        validate_safe_name(&current)?;
        if !seen.insert(current.clone()) {
            return Err(MinecraftError::CircularInheritance(current));
        }
        let json = load_version_json(paths, &current, manifest_url, fetch).await?;
        let next = json
            .get("inheritsFrom")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        chain.push(json);
        match next {
            Some(n) => current = n,
            None => break,
        }
    }
    let parent_id = chain
        .first()
        .and_then(|v| v.get("inheritsFrom"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mut iter = chain.into_iter().rev();
    let mut acc = iter.next().unwrap_or(Value::Null);
    for child in iter {
        acc = merge_version_json(&child, &acc);
    }
    Ok(ResolvedVersion {
        version: acc,
        parent_id,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn library_key_ignores_version_keeps_classifier() {
        assert_eq!(library_key("org.ow2.asm:asm:9.6"), "org.ow2.asm:asm");
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.3:natives-linux"),
            "org.lwjgl:lwjgl:natives-linux"
        );
    }

    #[test]
    fn child_libraries_win_and_arguments_concatenate() {
        let parent = json!({
            "id": "1.21.8", "mainClass": "net.minecraft.client.main.Main",
            "libraries": [
                {"name": "org.ow2.asm:asm:9.6"},
                {"name": "com.mojang:logging:1.0"},
                {"name": "org.lwjgl:lwjgl:3.3.3"},
                {"name": "org.lwjgl:lwjgl:3.3.3:natives-linux"}
            ],
            "arguments": {"game": ["--a"], "jvm": ["-Dp=1"]},
            "downloads": {"client": {"url": "u", "size": 1}},
            "assets": "17", "assetIndex": {"id": "17"},
            "javaVersion": {"majorVersion": 21}
        });
        let child = json!({
            "id": "fabric-loader-0.16-1.21.8", "inheritsFrom": "1.21.8",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{"name": "org.ow2.asm:asm:9.10.1"}],
            "arguments": {"game": ["--b"], "jvm": ["-Dc=2"]}
        });
        let m = merge_version_json(&child, &parent);
        let names: Vec<&str> = m["libraries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["name"].as_str().unwrap())
            .collect();
        assert_eq!(names[0], "org.ow2.asm:asm:9.10.1");
        assert!(!names.contains(&"org.ow2.asm:asm:9.6"));
        assert_eq!(names.len(), 4);
        assert_eq!(m["arguments"]["game"], json!(["--a", "--b"]));
        assert_eq!(m["arguments"]["jvm"], json!(["-Dp=1", "-Dc=2"]));
        assert_eq!(m["id"], "fabric-loader-0.16-1.21.8");
        assert_eq!(
            m["mainClass"],
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert!(m.get("inheritsFrom").is_none());
        assert_eq!(m["downloads"]["client"]["size"], 1);
        assert_eq!(m["javaVersion"]["majorVersion"], 21);
    }

    #[test]
    fn legacy_minecraft_arguments_inherited() {
        let m = merge_version_json(
            &json!({"id": "c", "inheritsFrom": "p"}),
            &json!({"id": "p", "minecraftArguments": "--x ${y}"}),
        );
        assert_eq!(m["minecraftArguments"], "--x ${y}");
        assert!(m.get("arguments").is_none());
    }

    #[test]
    fn unsafe_ids_rejected() {
        assert!(validate_safe_name("1.21.1").is_ok());
        assert!(validate_safe_name("fabric-loader-0.16.0-1.21").is_ok());
        for bad in ["", "..", "a/b", "a\\b", "x:y"] {
            assert!(validate_safe_name(bad).is_err(), "{bad}");
        }
    }
}

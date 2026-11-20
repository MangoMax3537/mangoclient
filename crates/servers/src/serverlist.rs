//! `servers.dat` reading, writing, partner seeding and icon capture.

use crate::error::{Result, ServersError};
use crate::fsutil::write_file_atomic;
use crate::icons::{normalize_address, IconStore};
use crate::nbt::{self, Entry, Root, Value};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A partnered server shown first in the player's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturedServer {
    pub id: String,
    pub name: String,
    pub address: String,
}

/// The default partner list.
pub fn featured_servers() -> Vec<FeaturedServer> {
    vec![FeaturedServer {
        id: "vincentvanilla".into(),
        name: "VincentVanilla".into(),
        address: "vincentvanilla.net".into(),
    }]
}

/// Gold star (Mango accent lookalike), reset so only the star is coloured.
pub fn featured_name(name: &str) -> String {
    format!("§6★ §r{name}")
}

fn sidecar(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(".mangoclient-backup");
    PathBuf::from(s)
}

/// An empty `{servers: []}` document.
pub fn empty_server_list() -> Root {
    Root {
        name: String::new(),
        entries: vec![servers_entry(Vec::new())],
    }
}

fn servers_entry(items: Vec<Value>) -> Entry {
    Entry::new(
        "servers",
        Value::List {
            element_type: nbt::TAG_COMPOUND,
            items,
        },
    )
}

async fn read_server_list_file(file: &Path) -> Result<Root> {
    let data = tokio::fs::read(file).await?;
    nbt::parse_nbt_maybe_gzip(&data)
}

/// Read servers.dat, falling back to the launcher sidecar on any failure. A
/// missing file with no sidecar yields an empty list.
pub async fn read_server_list(file: &Path) -> Result<Root> {
    match read_server_list_file(file).await {
        Ok(root) => Ok(root),
        Err(main_err) => match read_server_list_file(&sidecar(file)).await {
            Ok(root) => Ok(root),
            Err(_) => match &main_err {
                ServersError::Io(e) if e.kind() == ErrorKind::NotFound => Ok(empty_server_list()),
                _ => Err(main_err),
            },
        },
    }
}

/// Write the gzip NBT: sidecar first so an interrupted update always leaves
/// one complete copy, then servers.dat.
pub async fn write_server_list(file: &Path, root: &Root) -> Result<()> {
    let contents = nbt::encode_nbt_gzip(root)?;
    let file = file.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        write_file_atomic(&sidecar(&file), &contents)?;
        write_file_atomic(&file, &contents)?;
        Ok(())
    })
    .await
    .map_err(|_| ServersError::Task)?
}

fn str_field<'a>(compound: &'a [Entry], name: &str) -> Option<&'a str> {
    nbt::field(compound, name).and_then(|e| e.value.as_str())
}

fn item_address(item: &Value) -> String {
    match item {
        Value::Compound(c) => normalize_address(str_field(c, "ip").unwrap_or("")),
        _ => String::new(),
    }
}

fn set_string(compound: &mut Vec<Entry>, name: &str, value: String) {
    match compound.iter_mut().find(|e| e.name == name) {
        Some(e) => e.value = Value::String(value),
        None => compound.push(Entry::string(name, value)),
    }
}

/// Put partner servers first, retaining existing entries (icons, flags) and
/// deduplicating by normalized address. Rewrites servers.dat.
pub async fn ensure_featured_servers(
    game_dir: &Path,
    servers: &[FeaturedServer],
    icons: &dyn IconStore,
) -> Result<PathBuf> {
    let file = game_dir.join("servers.dat");
    let mut root = read_server_list(&file).await?;
    let idx = match root
        .entries
        .iter()
        .position(|e| e.name == "servers" && e.value.tag() == nbt::TAG_LIST)
    {
        Some(i) => i,
        None => {
            root.entries.push(servers_entry(Vec::new()));
            root.entries.len() - 1
        }
    };
    let Value::List {
        element_type,
        items,
    } = &mut root.entries[idx].value
    else {
        return Err(ServersError::InvalidServerList);
    };
    if *element_type != nbt::TAG_COMPOUND && !items.is_empty() {
        return Err(ServersError::InvalidServerList);
    }
    *element_type = nbt::TAG_COMPOUND;

    let featured_addrs: Vec<String> = servers
        .iter()
        .map(|s| normalize_address(&s.address))
        .collect();
    let mut featured = Vec::new();
    for server in servers {
        let addr = normalize_address(&server.address);
        let mut existing = match items.iter().find(|i| item_address(i) == addr) {
            Some(Value::Compound(c)) => c.clone(),
            _ => Vec::new(),
        };
        set_string(&mut existing, "name", featured_name(&server.name));
        set_string(&mut existing, "ip", server.address.clone());
        if let Some(icon) = icons.get(&server.address) {
            if str_field(&existing, "icon").unwrap_or("").is_empty() {
                set_string(&mut existing, "icon", icon);
            }
        }
        featured.push(Value::Compound(existing));
    }
    let rest = items
        .drain(..)
        .filter(|i| !featured_addrs.contains(&item_address(i)));
    let mut merged = featured;
    merged.extend(rest);
    *items = merged;
    write_server_list(&file, &root).await?;
    Ok(file)
}

/// Read Minecraft's final file (no sidecar fallback) and remember partner
/// favicons. Never writes the player's list.
pub async fn cache_featured_server_icons(
    game_dir: &Path,
    servers: &[FeaturedServer],
    icons: &dyn IconStore,
) -> Result<PathBuf> {
    let file = game_dir.join("servers.dat");
    let root = read_server_list_file(&file).await?;
    let Some(Value::List {
        element_type,
        items,
    }) = nbt::field(&root.entries, "servers")
        .filter(|e| e.value.tag() == nbt::TAG_LIST)
        .map(|e| &e.value)
    else {
        return Ok(file);
    };
    if *element_type != nbt::TAG_COMPOUND {
        return Ok(file);
    }
    let addrs: Vec<String> = servers
        .iter()
        .map(|s| normalize_address(&s.address))
        .collect();
    for item in items {
        let addr = item_address(item);
        if !addrs.contains(&addr) {
            continue;
        }
        if let Value::Compound(c) = item {
            if let Some(icon) = str_field(c, "icon").filter(|i| !i.is_empty()) {
                icons.remember(&addr, icon);
            }
        }
    }
    Ok(file)
}

/// Bootstrap a brand-new instance only. An existing servers.dat is never
/// rewritten; its partner icons are cached instead.
pub async fn seed_featured_servers(
    game_dir: &Path,
    servers: &[FeaturedServer],
    icons: &dyn IconStore,
) -> Result<PathBuf> {
    let file = game_dir.join("servers.dat");
    match tokio::fs::metadata(&file).await {
        Ok(_) => {
            let _ = cache_featured_server_icons(game_dir, servers, icons).await;
            Ok(file)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            ensure_featured_servers(game_dir, servers, icons).await
        }
        Err(e) => Err(e.into()),
    }
}

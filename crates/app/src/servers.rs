//! `servers_*` commands. Read-only on `servers.dat`: the player's list is
//! never rewritten here. What the user changes in the launcher (added
//! servers, renames, stars, removals) lives in `servers-user.json` next to the
//! other launcher files and is layered over the list.

use crate::error::{AppError, Res};
use crate::events::Event;
use crate::image_protocol::{file_url, server_icons_dir, sniff_extension};
use crate::models::{ServerEntry, ServerPlayers, ServerStatus, VersionRange};
use crate::service::AppService;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use mango_servers::{
    nbt, normalize_address, ping_all, ping_server_cached, read_server_list, strip_color_codes,
    FeaturedServer, IconStore,
};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

const MAX_ICON_BYTES: usize = 2 * 1024 * 1024;

/// A server as read from `servers.dat`.
struct ListedServer {
    name: String,
    address: String,
    icon: Option<String>,
}

/// Launcher-side edits of the server list (`servers-user.json`).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
struct UserServers {
    /// Servers added in the launcher, in the order they were added.
    custom: Vec<CustomServer>,
    /// Normalized addresses the user starred.
    starred: Vec<String>,
    /// Normalized addresses of `servers.dat` entries the user removed here.
    hidden: Vec<String>,
    /// Display names the user changed, by normalized address.
    names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomServer {
    name: String,
    address: String,
}

/// Serializes read-modify-write of `servers-user.json`.
static USER_LOCK: Mutex<()> = Mutex::new(());

fn user_file(paths: &mango_core::Paths) -> std::path::PathBuf {
    paths.root().join("servers-user.json")
}

fn load_user(paths: &mango_core::Paths) -> UserServers {
    mango_core::persistence::read_json_or(&user_file(paths), UserServers::default())
}

fn save_user(paths: &mango_core::Paths, u: &UserServers) -> Res<()> {
    mango_core::persistence::write_json_atomic(&user_file(paths), u)?;
    Ok(())
}

fn valid_address(address: &str) -> bool {
    let a = address.trim();
    !a.is_empty() && a.len() <= 255 && !a.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn clean_name(name: &str) -> Option<String> {
    let n: String = name.chars().filter(|c| !c.is_control()).collect();
    let n = n.trim();
    (!n.is_empty() && n.chars().count() <= 64).then(|| n.to_string())
}

fn user_id(address: &str) -> String {
    format!("user:{}", normalize_address(address))
}

/// Write the PNG behind a `servers.dat` / store icon (raw base64 or a
/// `data:` URL) into `cache/server-icons/` and return its `mangoimg://` URL.
fn materialize_icon(dir: &Path, raw: &str) -> Option<String> {
    let b64 = raw
        .trim()
        .strip_prefix("data:image/png;base64,")
        .unwrap_or_else(|| raw.trim());
    if b64.is_empty() || b64.len() > MAX_ICON_BYTES * 2 {
        return None;
    }
    let data = STANDARD.decode(b64.replace(['\n', '\r'], "")).ok()?;
    if data.len() > MAX_ICON_BYTES || sniff_extension(&data) != Some("png") {
        return None;
    }
    let file = dir.join(format!("{}.png", hex::encode(Sha1::digest(&data))));
    if !file.is_file() {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::write(&file, &data).ok()?;
    }
    Some(file_url(&file))
}

fn listed_servers(root: &nbt::Root) -> Vec<ListedServer> {
    let Some(nbt::Value::List { items, .. }) = nbt::field(&root.entries, "servers")
        .filter(|e| e.value.tag() == nbt::TAG_LIST)
        .map(|e| &e.value)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let nbt::Value::Compound(c) = item else {
                return None;
            };
            let text = |k: &str| {
                nbt::field(c, k)
                    .and_then(|e| e.value.as_str())
                    .map(str::to_string)
            };
            let address = text("ip").filter(|a| !a.trim().is_empty())?;
            Some(ListedServer {
                name: text("name").unwrap_or_default(),
                address,
                icon: text("icon").filter(|i| !i.is_empty()),
            })
        })
        .collect()
}

impl AppService {
    /// Partner servers first (starred), then the selected profile's own list.
    pub async fn servers_list(&self) -> Res<Vec<ServerEntry>> {
        let paths = self.inner.paths.clone();
        let selected = self
            .inner
            .with_store(|s| s.selected_profile().map(|p| p.id.clone()));
        let listed = match selected.and_then(|id| paths.instance_dir(&id).ok()) {
            Some(dir) => match read_server_list(&dir.join("servers.dat")).await {
                Ok(root) => listed_servers(&root),
                Err(e) => {
                    tracing::warn!("servers.dat unreadable: {e}");
                    Vec::new()
                }
            },
            None => Vec::new(),
        };
        let icons = Arc::clone(&self.inner.icons);
        let partners = self.inner.opts.partners.clone();
        Ok(tokio::task::spawn_blocking(move || {
            let user = load_user(&paths);
            build_entries(&paths, &partners, &listed, &user, icons.as_ref())
        })
        .await?)
    }

    /// Add a server of the user's own (name + address); returns the new list.
    pub async fn servers_add(&self, name: String, address: String) -> Res<Vec<ServerEntry>> {
        let name =
            clean_name(&name).ok_or_else(|| AppError::new("servers.name", "Invalid name"))?;
        if !valid_address(&address) {
            return Err(AppError::new("servers.address", "Invalid server address"));
        }
        let address = address.trim().to_string();
        let key = normalize_address(&address);
        let current = self.servers_list().await?;
        let paths = self.inner.paths.clone();
        {
            let _g = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut u = load_user(&paths);
            let listed = current.iter().any(|e| normalize_address(&e.address) == key);
            if listed {
                return Err(AppError::new(
                    "servers.duplicate",
                    "This server is already in the list",
                ));
            }
            // A previously removed servers.dat entry comes back as itself.
            if u.hidden.contains(&key) {
                u.hidden.retain(|h| *h != key);
            }
            u.custom.push(CustomServer { name, address });
            save_user(&paths, &u)?;
        }
        self.servers_list().await
    }

    /// Rename a server; a launcher-added one can change its address too.
    pub async fn servers_update(
        &self,
        id: String,
        name: String,
        address: Option<String>,
    ) -> Res<Vec<ServerEntry>> {
        let name =
            clean_name(&name).ok_or_else(|| AppError::new("servers.name", "Invalid name"))?;
        let current = self.servers_list().await?;
        let entry = current
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("servers.not_found", "Unknown server"))?;
        if entry.partner {
            return Err(AppError::new(
                "servers.partner",
                "Partner servers are fixed",
            ));
        }
        let old = normalize_address(&entry.address);
        let new_address = match address.map(|a| a.trim().to_string()) {
            Some(a) if entry.custom && normalize_address(&a) != old => {
                if !valid_address(&a) {
                    return Err(AppError::new("servers.address", "Invalid server address"));
                }
                let key = normalize_address(&a);
                if current.iter().any(|e| normalize_address(&e.address) == key) {
                    return Err(AppError::new(
                        "servers.duplicate",
                        "This server is already in the list",
                    ));
                }
                Some(a)
            }
            _ => None,
        };
        let paths = self.inner.paths.clone();
        let custom = entry.custom;
        {
            let _g = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut u = load_user(&paths);
            if custom {
                if let Some(c) = u
                    .custom
                    .iter_mut()
                    .find(|c| normalize_address(&c.address) == old)
                {
                    c.name = name;
                    if let Some(a) = &new_address {
                        c.address = a.clone();
                        if u.starred.contains(&old) {
                            u.starred.retain(|s| *s != old);
                            u.starred.push(normalize_address(a));
                        }
                    }
                }
            } else {
                u.names.insert(old, name);
            }
            save_user(&paths, &u)?;
        }
        self.servers_list().await
    }

    /// Remove a server from the list. `servers.dat` is not touched: an entry
    /// from it is hidden in the launcher instead. Partners cannot go.
    pub async fn servers_remove(&self, id: String) -> Res<Vec<ServerEntry>> {
        let current = self.servers_list().await?;
        let entry = current
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("servers.not_found", "Unknown server"))?;
        if entry.partner {
            return Err(AppError::new(
                "servers.partner",
                "Partner servers are fixed",
            ));
        }
        let key = normalize_address(&entry.address);
        let custom = entry.custom;
        let paths = self.inner.paths.clone();
        {
            let _g = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut u = load_user(&paths);
            u.starred.retain(|s| *s != key);
            u.names.remove(&key);
            if custom {
                u.custom.retain(|c| normalize_address(&c.address) != key);
            } else if !u.hidden.contains(&key) {
                u.hidden.push(key);
            }
            save_user(&paths, &u)?;
        }
        self.servers_list().await
    }

    /// Star or unstar a server (partners stay pinned and cannot change).
    pub async fn servers_star(&self, id: String, starred: bool) -> Res<Vec<ServerEntry>> {
        let current = self.servers_list().await?;
        let entry = current
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("servers.not_found", "Unknown server"))?;
        if entry.partner {
            return Err(AppError::new(
                "servers.partner",
                "Partner servers are fixed",
            ));
        }
        let key = normalize_address(&entry.address);
        let paths = self.inner.paths.clone();
        {
            let _g = USER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut u = load_user(&paths);
            u.starred.retain(|s| *s != key);
            if starred {
                u.starred.push(key);
            }
            save_user(&paths, &u)?;
        }
        self.servers_list().await
    }

    /// Every server the list shows, as ping targets.
    async fn ping_targets(&self) -> Res<Vec<FeaturedServer>> {
        Ok(self
            .servers_list()
            .await?
            .into_iter()
            .map(|e| FeaturedServer {
                id: e.id,
                name: e.name,
                address: e.address,
            })
            .collect())
    }

    /// Returns immediately; each result arrives as `servers:pinged`.
    pub async fn servers_ping_all(&self) -> Res<()> {
        let targets = self.ping_targets().await?;
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let opts = inner.opts.ping.clone();
            let icons: Arc<dyn IconStore> = inner.icons.clone();
            let emit = Arc::clone(&inner);
            let dir = server_icons_dir(&inner.paths);
            ping_all(
                &targets,
                &opts,
                mango_servers::default_resolver(),
                icons,
                move |p| {
                    emit.emit(Event::ServersPinged(status_of(&dir, &p.id, &p.status)));
                },
            )
            .await;
        });
        Ok(())
    }

    pub async fn servers_ping(&self, address: String) -> Res<ServerStatus> {
        let address = address.trim().to_string();
        if !valid_address(&address) {
            return Err(AppError::validation("Invalid server address"));
        }
        let id = self
            .inner
            .opts
            .partners
            .iter()
            .find(|f| normalize_address(&f.address) == normalize_address(&address))
            .map_or_else(|| user_id(&address), |f| f.id.clone());
        let status = ping_server_cached(
            &address,
            &self.inner.opts.ping,
            mango_servers::default_resolver().as_ref(),
            self.inner.icons.as_ref(),
        )
        .await;
        let dir = server_icons_dir(&self.inner.paths);
        Ok(status_of(&dir, &id, &status))
    }
}

fn status_of(dir: &Path, id: &str, s: &mango_servers::ServerStatus) -> ServerStatus {
    ServerStatus {
        id: id.to_string(),
        online: s.online,
        ping: s.ping_ms,
        motd: s.motd.clone(),
        players: s.players.as_ref().map(|p| ServerPlayers {
            online: p.online,
            max: p.max,
        }),
        version: Some(s.version.clone()).filter(|v| !v.is_empty()),
        version_range: mango_servers::parse_version_range(&s.version, s.protocol)
            .filter(|_| s.online)
            .map(|r| VersionRange {
                min: r.min,
                max: r.max,
            }),
        // `ping_server_cached` already swapped in the durable first favicon.
        icon_url: s
            .favicon
            .as_deref()
            .and_then(|raw| materialize_icon(dir, raw)),
    }
}

fn build_entries(
    paths: &mango_core::Paths,
    partners: &[FeaturedServer],
    listed: &[ListedServer],
    user: &UserServers,
    icons: &dyn IconStore,
) -> Vec<ServerEntry> {
    let dir = server_icons_dir(paths);
    let nbt_icon = |address: &str| {
        let key = normalize_address(address);
        listed
            .iter()
            .find(|l| normalize_address(&l.address) == key)
            .and_then(|l| l.icon.clone())
    };
    let mut out = Vec::new();
    for f in partners {
        let icon = icons
            .get(&f.address)
            .or_else(|| nbt_icon(&f.address))
            .and_then(|raw| materialize_icon(&dir, &raw));
        out.push(ServerEntry {
            id: f.id.clone(),
            name: f.name.clone(),
            address: f.address.clone(),
            partner: true,
            starred: true,
            custom: false,
            icon_url: icon,
        });
    }
    let taken: Vec<String> = partners
        .iter()
        .map(|f| normalize_address(&f.address))
        .collect();
    let mut seen: Vec<String> = Vec::new();
    for l in listed {
        let key = normalize_address(&l.address);
        if taken.contains(&key) || seen.contains(&key) {
            continue;
        }
        if user.hidden.contains(&key) {
            continue;
        }
        seen.push(key.clone());
        let name = user
            .names
            .get(&key)
            .cloned()
            .unwrap_or_else(|| strip_color_codes(&l.name).trim().to_string());
        let icon = l
            .icon
            .clone()
            .or_else(|| icons.get(&l.address))
            .and_then(|raw| materialize_icon(&dir, &raw));
        out.push(ServerEntry {
            id: user_id(&l.address),
            name: if name.is_empty() {
                l.address.trim().to_string()
            } else {
                name
            },
            address: l.address.trim().to_string(),
            partner: false,
            starred: user.starred.contains(&key),
            custom: false,
            icon_url: icon,
        });
    }
    for c in &user.custom {
        let key = normalize_address(&c.address);
        if taken.contains(&key) || seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());
        let icon = icons
            .get(&c.address)
            .and_then(|raw| materialize_icon(&dir, &raw));
        out.push(ServerEntry {
            id: user_id(&c.address),
            name: c.name.clone(),
            address: c.address.trim().to_string(),
            partner: false,
            starred: user.starred.contains(&key),
            custom: true,
            icon_url: icon,
        });
    }
    out
}

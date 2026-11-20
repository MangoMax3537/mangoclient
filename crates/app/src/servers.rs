//! `servers_*` commands. Read-only on `servers.dat`: the player's list is
//! never rewritten here.

use crate::error::{AppError, Res};
use crate::events::Event;
use crate::image_protocol::{file_url, server_icons_dir, sniff_extension};
use crate::models::{ServerEntry, ServerPlayers, ServerStatus};
use crate::service::AppService;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use mango_servers::{
    nbt, normalize_address, ping_all, ping_server_cached, read_server_list, strip_color_codes,
    FeaturedServer, IconStore, NoSrv,
};
use sha1::{Digest, Sha1};
use std::path::Path;
use std::sync::Arc;

const MAX_ICON_BYTES: usize = 2 * 1024 * 1024;

/// A server as read from `servers.dat`.
struct ListedServer {
    name: String,
    address: String,
    icon: Option<String>,
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
            build_entries(&paths, &partners, &listed, icons.as_ref())
        })
        .await?)
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
            ping_all(&targets, &opts, Arc::new(NoSrv), icons, move |p| {
                emit.emit(Event::ServersPinged(status_of(&p.id, &p.status)));
            })
            .await;
        });
        Ok(())
    }

    pub async fn servers_ping(&self, address: String) -> Res<ServerStatus> {
        let address = address.trim().to_string();
        if address.is_empty()
            || address.len() > 255
            || address.chars().any(|c| c.is_whitespace() || c.is_control())
        {
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
            &NoSrv,
            self.inner.icons.as_ref(),
        )
        .await;
        Ok(status_of(&id, &status))
    }
}

fn status_of(id: &str, s: &mango_servers::ServerStatus) -> ServerStatus {
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
    }
}

fn build_entries(
    paths: &mango_core::Paths,
    partners: &[FeaturedServer],
    listed: &[ListedServer],
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
        seen.push(key);
        let name = strip_color_codes(&l.name).trim().to_string();
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
            starred: false,
            icon_url: icon,
        });
    }
    out
}

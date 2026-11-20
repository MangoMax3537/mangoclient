//! mango-servers: servers.dat NBT, partner seeding, favicon store and Server
//! List Ping. Port of `serverlist.js`, `servers.js` and `servericons.js`.

mod error;
mod fsutil;
pub mod icons;
pub mod nbt;
pub mod ping;
pub mod serverlist;

pub use error::{Result, ServersError};
pub use icons::{normalize_address, valid_icon, IconStore, NoIcons, ServerIcons};
pub use ping::{
    flatten_motd, ping_all, ping_server, ping_server_cached, ping_server_with, strip_color_codes,
    NoSrv, PingOptions, PingedServer, Players, ServerStatus, SrvResolver,
};
pub use serverlist::{
    cache_featured_server_icons, empty_server_list, ensure_featured_servers, featured_name,
    featured_servers, read_server_list, seed_featured_servers, write_server_list, FeaturedServer,
};

#[cfg(test)]
mod tests;

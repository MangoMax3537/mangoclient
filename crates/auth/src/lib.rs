//! Microsoft/Xbox/Minecraft sign-in, offline accounts, the credential vault
//! and skin handling.
//!
//! Port of `src/main/auth.js`, `credentials.js` and the network/cache parts of
//! `skins.js`. No UI and no Tauri: the caller opens the sign-in window, feeds
//! navigations into [`Endpoints::parse_redirect`] and passes the code on.
//! Tokens never appear in `Debug` output or in [`PublicAccount`]. Token
//! storage design and threat model: see the `vault` module docs (re-exported
//! as [`Vault`]).

mod account;
mod endpoints;
mod envelope;
mod error;
mod http;
mod keystore;
mod msa;
mod offline;
mod skins;
mod store;
mod vault;

#[cfg(test)]
mod tests;

pub use account::{Account, AccountType, PublicAccount, Texture};
pub use endpoints::{new_oauth_state, Endpoints, Redirect};
pub use envelope::{open as open_envelope, seal as seal_envelope, ENVELOPE_V2_PREFIX};
pub use error::{AuthError, StoreError};
#[cfg(feature = "keyring")]
pub use keystore::KeyringDeviceKey;
pub use keystore::{DeviceKey, DeviceKeyStore, MemoryDeviceKey};
pub use msa::{
    ensure_fresh_account, exchange_code, login_with_tokens, minecraft_login, refresh, MsaTokens,
};
pub use offline::offline_account;
pub use skins::{
    default_skin_png, fetch_profile_textures, fetch_public_skin, fetch_texture_bytes,
    get_cached_skin, get_skin, lookup_player_uuid, set_active_cape, upload_skin, upload_skin_bytes,
    ProfileTextures, SkinResult, SkinSource, SkinVariant,
};
pub use store::CredentialStore;
#[cfg(feature = "keyring")]
pub use store::KeyringStore;
pub use vault::{
    has_plaintext_credentials, is_legacy_key_name, needs_migration, StoredAccount, Vault,
    ENVELOPE_PREFIX, SECRET_FIELDS,
};

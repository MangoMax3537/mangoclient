//! Microsoft/Xbox/Minecraft sign-in, offline accounts, the credential vault
//! and skin handling.
//!
//! Port of `src/main/auth.js`, `credentials.js` and the network/cache parts of
//! `skins.js`. No UI and no Tauri: the caller opens the sign-in window, feeds
//! navigations into [`Endpoints::parse_redirect`] and passes the code on.
//! Tokens never appear in `Debug` output or in [`PublicAccount`].

mod account;
mod endpoints;
mod error;
mod http;
mod msa;
mod offline;
mod skins;
mod store;
mod vault;

#[cfg(test)]
mod tests;

pub use account::{Account, AccountType, PublicAccount, Texture};
pub use endpoints::{Endpoints, Redirect};
pub use error::{AuthError, StoreError};
pub use msa::{
    ensure_fresh_account, exchange_code, login_with_tokens, minecraft_login, refresh, MsaTokens,
};
pub use offline::offline_account;
pub use skins::{
    default_skin_png, get_cached_skin, get_skin, upload_skin, SkinResult, SkinSource, SkinVariant,
};
#[cfg(feature = "keyring")]
pub use store::KeyringStore;
pub use store::{CredentialStore, FileFallbackStore};
pub use vault::{
    delete_credentials, deserialize_account, has_plaintext_credentials, serialize_account,
    ENVELOPE_PREFIX, SECRET_FIELDS,
};

//! mango-core: paths, atomic JSON persistence, validation and the
#![cfg_attr(test, allow(clippy::unwrap_used))]
//! config/profile store. No UI, no Tauri.

pub mod error;
pub mod model;
pub mod paths;
pub mod persistence;
pub mod store;
pub mod validation;

pub use error::{Error, Result};
pub use model::{
    Account, AccountType, Config, Loader, ModDependency, ModRecord, ModType, Profile, Secret,
    Session,
};
pub use paths::{default_downloads_dir, downloads_dir_in, validate_profile_id, Paths};
pub use store::Store;
pub use validation::{config_patch, profile_patch, validate_mods, ValidationError};

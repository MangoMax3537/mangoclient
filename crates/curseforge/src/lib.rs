//! CurseForge Core API client, file selection and modpack import.
//!
//! Terms of use in short: the API key identifies the app, files whose author
//! disabled third-party distribution (`allowModDistribution == false`, or no
//! download URL) are never downloaded here and are reported with their
//! website address instead.
//!
//! Nothing in this crate persists profile data.

mod client;
mod download;
mod error;
mod html;
mod modpack;
mod select;
mod types;

#[cfg(test)]
mod tests;

pub use client::{is_trusted_cdn_url, ApiKey, Client, Options, API};
pub use download::download_cf_file;
pub use error::{Error, Result};
pub use html::html_to_text;
pub use modpack::{
    install_resolved, parse_loader_id, parse_manifest, read_manifest, resolve_pack, safe_filename,
    BlockedFile, Manifest, ManifestFile, PackEntry, PackProgress, ProgressFn, ResolvedPack,
    MAX_MANIFEST_BYTES,
};
pub use select::{dependencies_of, is_compatible, pick_best_file};
pub use types::{
    website_url, Asset, Author, Category, ClassId, File, FileDependency, FileHash, FileIndex,
    Links, ModLoaderType, Pagination, Project, Relation, ReleaseType, SearchQuery, SortField,
    GAME_ID,
};

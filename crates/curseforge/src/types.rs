//! Wire types of the CurseForge Core API v1 (only the fields we use).

use serde::{Deserialize, Serialize};

/// Minecraft's game id on CurseForge.
pub const GAME_ID: u32 = 432;

/// Content class of a project (`classId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassId {
    Mods,
    Modpacks,
    ResourcePacks,
    Shaders,
    DataPacks,
}

impl ClassId {
    pub fn id(self) -> u32 {
        match self {
            ClassId::Mods => 6,
            ClassId::Modpacks => 4471,
            ClassId::ResourcePacks => 12,
            ClassId::Shaders => 6552,
            ClassId::DataPacks => 6945,
        }
    }

    pub fn from_id(id: u32) -> Option<ClassId> {
        Some(match id {
            6 => ClassId::Mods,
            4471 => ClassId::Modpacks,
            12 => ClassId::ResourcePacks,
            6552 => ClassId::Shaders,
            6945 => ClassId::DataPacks,
            _ => return None,
        })
    }

    /// Path segment of `https://www.curseforge.com/minecraft/<segment>/<slug>`.
    pub fn site_segment(self) -> &'static str {
        match self {
            ClassId::Mods => "mc-mods",
            ClassId::Modpacks => "modpacks",
            ClassId::ResourcePacks => "texture-packs",
            ClassId::Shaders => "shaders",
            ClassId::DataPacks => "data-packs",
        }
    }
}

/// `modLoaderType`. Only the loaders this launcher can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModLoaderType {
    Forge,
    Fabric,
    Quilt,
    NeoForge,
}

impl ModLoaderType {
    pub fn id(self) -> u32 {
        match self {
            ModLoaderType::Forge => 1,
            ModLoaderType::Fabric => 4,
            ModLoaderType::Quilt => 5,
            ModLoaderType::NeoForge => 6,
        }
    }

    /// The loader name as it appears in a file's `gameVersions`.
    pub fn tag(self) -> &'static str {
        match self {
            ModLoaderType::Forge => "Forge",
            ModLoaderType::Fabric => "Fabric",
            ModLoaderType::Quilt => "Quilt",
            ModLoaderType::NeoForge => "NeoForge",
        }
    }

    /// From the launcher's loader name; `vanilla` and unknown names are `None`.
    pub fn from_name(name: &str) -> Option<ModLoaderType> {
        Some(match name.to_ascii_lowercase().as_str() {
            "forge" => ModLoaderType::Forge,
            "fabric" => ModLoaderType::Fabric,
            "quilt" => ModLoaderType::Quilt,
            "neoforge" => ModLoaderType::NeoForge,
            _ => return None,
        })
    }

    /// Loaders whose files also run here, best match first. Quilt loads Fabric
    /// mods and NeoForge loads (older) Forge ones.
    pub fn accepts(self) -> &'static [ModLoaderType] {
        match self {
            ModLoaderType::Quilt => &[ModLoaderType::Quilt, ModLoaderType::Fabric],
            ModLoaderType::NeoForge => &[ModLoaderType::NeoForge, ModLoaderType::Forge],
            ModLoaderType::Forge => &[ModLoaderType::Forge],
            ModLoaderType::Fabric => &[ModLoaderType::Fabric],
        }
    }
}

/// `sortField` values the launcher uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Relevance,
    Downloads,
    /// CurseForge has no follower count; popularity is the closest signal.
    Follows,
    Newest,
    Updated,
}

impl SortField {
    pub fn parse(s: &str) -> Option<SortField> {
        Some(match s {
            "relevance" => SortField::Relevance,
            "downloads" => SortField::Downloads,
            "follows" => SortField::Follows,
            "newest" => SortField::Newest,
            "updated" => SortField::Updated,
            _ => return None,
        })
    }

    /// `ModsSearchSortField`: 1 Featured, 2 Popularity, 3 LastUpdated,
    /// 6 TotalDownloads, 11 ReleasedDate.
    pub fn id(self) -> u32 {
        match self {
            SortField::Relevance => 1,
            SortField::Follows => 2,
            SortField::Updated => 3,
            SortField::Downloads => 6,
            SortField::Newest => 11,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub filter: String,
    pub class: ClassId,
    pub game_version: Option<String>,
    pub loader: Option<ModLoaderType>,
    pub sort: SortField,
    pub index: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    #[serde(default)]
    pub index: u64,
    #[serde(default)]
    pub page_size: u64,
    #[serde(default)]
    pub result_count: u64,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIndex {
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub file_id: u64,
    #[serde(default)]
    pub release_type: u32,
    #[serde(default)]
    pub mod_loader: Option<u32>,
}

/// Counts arrive as integers, occasionally as floats.
fn count<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_u64()
        .or_else(|| {
            v.as_f64()
                .map(|f| if f.is_sign_negative() { 0 } else { f as u64 })
        })
        .unwrap_or(0))
}

/// A CurseForge project ("mod" in API speak, whatever its class).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub links: Links,
    #[serde(default, deserialize_with = "count")]
    pub download_count: u64,
    #[serde(default)]
    pub thumbs_up_count: u64,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub class_id: Option<u32>,
    #[serde(default)]
    pub authors: Vec<Author>,
    #[serde(default)]
    pub logo: Option<Asset>,
    #[serde(default)]
    pub screenshots: Vec<Asset>,
    #[serde(default)]
    pub latest_files: Vec<File>,
    #[serde(default)]
    pub latest_files_indexes: Vec<FileIndex>,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub date_released: String,
    /// `false` = third-party launchers must not download this project's files.
    #[serde(default)]
    pub allow_mod_distribution: Option<bool>,
    #[serde(default)]
    pub is_available: Option<bool>,
}

impl Project {
    pub fn class(&self) -> Option<ClassId> {
        self.class_id.and_then(ClassId::from_id)
    }

    /// The project's distribution is explicitly forbidden.
    pub fn distribution_blocked(&self) -> bool {
        self.allow_mod_distribution == Some(false)
    }

    /// `https://www.curseforge.com/minecraft/<class>/<slug>`, built by us so an
    /// API answer can never point the UI at another site. `None` when the slug
    /// is not a plain slug or the class is unknown.
    pub fn website_url(&self) -> Option<String> {
        website_url(self.class()?, &self.slug)
    }
}

/// See [`Project::website_url`].
pub fn website_url(class: ClassId, slug: &str) -> Option<String> {
    let plain = !slug.is_empty()
        && slug.len() <= 128
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    plain.then(|| {
        format!(
            "https://www.curseforge.com/minecraft/{}/{slug}",
            class.site_segment()
        )
    })
}

/// `FileReleaseType`: 1 release, 2 beta, 3 alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReleaseType {
    Release,
    Beta,
    Alpha,
}

impl ReleaseType {
    pub fn from_id(id: u32) -> ReleaseType {
        match id {
            1 => ReleaseType::Release,
            2 => ReleaseType::Beta,
            _ => ReleaseType::Alpha,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ReleaseType::Release => "release",
            ReleaseType::Beta => "beta",
            ReleaseType::Alpha => "alpha",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHash {
    #[serde(default)]
    pub value: String,
    /// 1 = SHA-1, 2 = MD5.
    #[serde(default)]
    pub algo: u32,
}

/// `FileRelationType`: 1 embedded library, 2 optional, 3 required, 4 tool,
/// 5 incompatible, 6 include.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Embedded,
    Optional,
    Required,
    Tool,
    Incompatible,
    Include,
    Unknown,
}

impl Relation {
    pub fn from_id(id: u32) -> Relation {
        match id {
            1 => Relation::Embedded,
            2 => Relation::Optional,
            3 => Relation::Required,
            4 => Relation::Tool,
            5 => Relation::Incompatible,
            6 => Relation::Include,
            _ => Relation::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDependency {
    #[serde(default)]
    pub mod_id: u64,
    #[serde(default)]
    pub relation_type: u32,
}

impl FileDependency {
    pub fn relation(&self) -> Relation {
        Relation::from_id(self.relation_type)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: u64,
    #[serde(default)]
    pub mod_id: u64,
    #[serde(default)]
    pub is_available: Option<bool>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub release_type: u32,
    #[serde(default)]
    pub hashes: Vec<FileHash>,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub download_count: u64,
    /// `null` when the author opted out of third-party downloads.
    #[serde(default)]
    pub download_url: Option<String>,
    /// Loader names and Minecraft versions (plus `Client`/`Server`/`Java 21`).
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<FileDependency>,
    #[serde(default)]
    pub is_server_pack: Option<bool>,
    #[serde(default)]
    pub file_fingerprint: Option<u64>,
}

impl File {
    pub fn release(&self) -> ReleaseType {
        ReleaseType::from_id(self.release_type)
    }

    /// Lower-case hex SHA-1 when the API supplied one.
    pub fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|h| h.algo == 1 && !h.value.is_empty())
            .map(|h| h.value.to_ascii_lowercase())
    }

    pub fn is_available(&self) -> bool {
        self.is_available != Some(false)
    }

    /// Minecraft versions this file declares (everything that looks like one).
    pub fn minecraft_versions(&self) -> impl Iterator<Item = &str> {
        self.game_versions
            .iter()
            .map(String::as_str)
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
    }

    /// Loaders this file declares.
    pub fn loaders(&self) -> Vec<ModLoaderType> {
        [
            ModLoaderType::Forge,
            ModLoaderType::Fabric,
            ModLoaderType::Quilt,
            ModLoaderType::NeoForge,
        ]
        .into_iter()
        .filter(|l| {
            self.game_versions
                .iter()
                .any(|g| g.eq_ignore_ascii_case(l.tag()))
        })
        .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Envelope<T> {
    pub data: T,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

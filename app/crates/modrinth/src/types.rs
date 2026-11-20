use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

fn mod_type() -> String {
    "mod".into()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hashes {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VersionFile {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub hashes: Hashes,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "mod_type")]
    pub project_type: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Search arguments. `Default` matches the JS defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchParams {
    pub query: String,
    pub loader: String,
    pub game_version: String,
    pub project_type: String,
    pub categories: Vec<String>,
    pub limit: u32,
    pub offset: u32,
    pub index: String,
    pub open_source_only: bool,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            loader: "fabric".into(),
            game_version: String::new(),
            project_type: "mod".into(),
            categories: Vec::new(),
            limit: 30,
            offset: 0,
            index: "relevance".into(),
            open_source_only: false,
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub collections: Vec<WallhavenCollection>,
    #[serde(default)]
    pub search: WallhavenSearch,
    #[serde(default = "default_prefer")]
    pub prefer: WallhavenPrefer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenCollection {
    pub username: String,
    pub id: u32,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenSearch {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_categories")]
    pub categories: String,
    #[serde(default = "default_purity")]
    pub purity: String,
    #[serde(default = "default_sorting")]
    pub sorting: String,
    #[serde(default = "default_order")]
    pub order: String,
    #[serde(default = "default_atleast")]
    pub atleast: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WallhavenPrefer {
    CollectionsThenSearch,
    SearchOnly,
    CollectionsOnly,
}

fn default_enabled() -> bool {
    true
}

impl Default for WallhavenConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collections: Vec::new(),
            search: WallhavenSearch::default(),
            prefer: WallhavenPrefer::CollectionsThenSearch,
        }
    }
}

impl Default for WallhavenSearch {
    fn default() -> Self {
        Self {
            q: String::new(),
            categories: default_categories(),
            purity: default_purity(),
            sorting: default_sorting(),
            order: default_order(),
            atleast: default_atleast(),
        }
    }
}

fn default_prefer() -> WallhavenPrefer {
    WallhavenPrefer::CollectionsThenSearch
}
fn default_categories() -> String {
    "111".into()
}
fn default_purity() -> String {
    "100".into()
}
fn default_sorting() -> String {
    "random".into()
}
fn default_order() -> String {
    "desc".into()
}
fn default_atleast() -> String {
    "1920x1080".into()
}

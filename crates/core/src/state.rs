use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::WallhavenSearch;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct State {
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub no_effects_on: Option<String>,
    #[serde(default)]
    pub current: Option<CurrentWall>,
    #[serde(default)]
    pub history: Vec<String>,
    #[serde(default)]
    pub history_index: usize,
    #[serde(default)]
    pub wallhaven: WallhavenState,
    #[serde(default)]
    pub cache_queue: Vec<String>,
    #[serde(default)]
    pub last_change_unix: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct CurrentWall {
    pub source_id: String,
    #[serde(default)]
    pub wallhaven_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub original_path: String,
    pub composed_path: String,
    #[serde(default)]
    pub post_filter_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CurrentWallMetadata {
    pub provider: Option<String>,
    pub source_url: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct WallhavenState {
    #[serde(default)]
    pub random_seed: Option<String>,
    #[serde(default)]
    pub collection_pages: std::collections::HashMap<String, u32>,
    #[serde(default)]
    pub search_page: u32,
    #[serde(default)]
    pub source_search_pages: std::collections::HashMap<String, u32>,
    #[serde(default)]
    pub effective_source_searches: std::collections::HashMap<String, WallhavenSearch>,
}

impl State {
    pub fn load_or_default(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(path)?;
        if data.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&data)?)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(tmp, path)?;
        Ok(())
    }
}

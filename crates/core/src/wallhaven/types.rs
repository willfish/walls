use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Wallpaper {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SearchResponse {
    pub data: Vec<Wallpaper>,
    pub meta: SearchMeta,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SearchMeta {
    pub current_page: u32,
    pub last_page: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WallpaperResponse {
    pub data: Wallpaper,
}

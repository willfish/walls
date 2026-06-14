use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Wallpaper {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub tags: Vec<WallpaperTag>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WallpaperTag {
    pub name: String,
    #[serde(default)]
    pub purity: Option<String>,
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

pub fn tag_names_from_wallpaper(wallpaper: &Wallpaper) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    wallpaper
        .tags
        .iter()
        .filter(|tag| tag.purity.as_deref().unwrap_or("sfw") == "sfw")
        .map(|tag| tag.name.trim())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert(tag.to_ascii_lowercase()))
        .take(5)
        .map(str::to_string)
        .collect()
}

pub fn tag_query_from_wallpaper(wallpaper: &Wallpaper) -> Option<String> {
    let tags = tag_names_from_wallpaper(wallpaper)
        .iter()
        .filter_map(|tag| crate::config::wallhaven_required_tag_query_part(tag))
        .collect::<Vec<_>>();

    if tags.is_empty() {
        None
    } else {
        Some(tags.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_query_from_wallpaper_uses_sfw_required_tags() {
        let wallpaper = Wallpaper {
            id: "yqxev7".into(),
            path: "https://example.test/wall.jpg".into(),
            tags: vec![
                WallpaperTag {
                    name: "blue eyes".into(),
                    purity: Some("sfw".into()),
                },
                WallpaperTag {
                    name: "skirt".into(),
                    purity: Some("sfw".into()),
                },
                WallpaperTag {
                    name: "girls with guns".into(),
                    purity: Some("sketchy".into()),
                },
            ],
        };

        assert_eq!(
            tag_query_from_wallpaper(&wallpaper).as_deref(),
            Some("+\"blue eyes\" +skirt")
        );
        assert_eq!(
            tag_names_from_wallpaper(&wallpaper),
            vec!["blue eyes", "skirt"]
        );
    }
}

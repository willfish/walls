#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Folder,
    Image,
    Favorites,
    Fetched,
    Unsplash,
    Reddit,
    Bing,
    Apod,
    MediaRss,
    Attribution,
    Json,
    Pixabay,
    Immich,
    Spotlight,
    Weighting,
    Wallhaven,
    Unknown,
}

impl SourceKind {
    pub fn parse(source_type: &str) -> Self {
        match source_type {
            "folder" => Self::Folder,
            "image" => Self::Image,
            "favorites" => Self::Favorites,
            "fetched" => Self::Fetched,
            "unsplash" => Self::Unsplash,
            "reddit" => Self::Reddit,
            "bing" => Self::Bing,
            "apod" => Self::Apod,
            "mediarss" => Self::MediaRss,
            "attribution" => Self::Attribution,
            "json" => Self::Json,
            "pixabay" => Self::Pixabay,
            "immich" => Self::Immich,
            "spotlight" => Self::Spotlight,
            "weighting" => Self::Weighting,
            "wallhaven" => Self::Wallhaven,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Folder => Some("folder"),
            Self::Image => Some("image"),
            Self::Favorites => Some("favorites"),
            Self::Fetched => Some("fetched"),
            Self::Unsplash => Some("unsplash"),
            Self::Reddit => Some("reddit"),
            Self::Bing => Some("bing"),
            Self::Apod => Some("apod"),
            Self::MediaRss => Some("mediarss"),
            Self::Attribution => Some("attribution"),
            Self::Json => Some("json"),
            Self::Pixabay => Some("pixabay"),
            Self::Immich => Some("immich"),
            Self::Spotlight => Some("spotlight"),
            Self::Weighting => Some("weighting"),
            Self::Wallhaven => Some("wallhaven"),
            Self::Unknown => None,
        }
    }

    pub fn is_local(self) -> bool {
        matches!(
            self,
            Self::Folder | Self::Image | Self::Favorites | Self::Fetched
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_local_source_kinds() {
        assert_eq!(SourceKind::parse("folder"), SourceKind::Folder);
        assert_eq!(SourceKind::parse("image"), SourceKind::Image);
        assert_eq!(SourceKind::parse("favorites"), SourceKind::Favorites);
        assert_eq!(SourceKind::parse("fetched"), SourceKind::Fetched);
        assert!(SourceKind::parse("folder").is_local());
    }

    #[test]
    fn parses_known_online_source_kinds() {
        assert_eq!(SourceKind::parse("unsplash"), SourceKind::Unsplash);
        assert_eq!(SourceKind::parse("reddit"), SourceKind::Reddit);
        assert_eq!(SourceKind::parse("bing"), SourceKind::Bing);
        assert_eq!(SourceKind::parse("apod"), SourceKind::Apod);
        assert_eq!(SourceKind::parse("mediarss"), SourceKind::MediaRss);
        assert_eq!(SourceKind::parse("attribution"), SourceKind::Attribution);
        assert_eq!(SourceKind::parse("json"), SourceKind::Json);
        assert_eq!(SourceKind::parse("pixabay"), SourceKind::Pixabay);
        assert_eq!(SourceKind::parse("immich"), SourceKind::Immich);
        assert_eq!(SourceKind::parse("spotlight"), SourceKind::Spotlight);
        assert_eq!(SourceKind::parse("weighting"), SourceKind::Weighting);
        assert_eq!(SourceKind::parse("wallhaven"), SourceKind::Wallhaven);
        assert!(!SourceKind::parse("unsplash").is_local());
    }

    #[test]
    fn unknown_source_kind_remains_representable() {
        assert_eq!(SourceKind::parse("future-provider"), SourceKind::Unknown);
        assert_eq!(SourceKind::parse("future-provider").as_str(), None);
    }
}

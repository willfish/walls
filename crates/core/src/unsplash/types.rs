use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Photo {
    pub id: String,
    pub urls: PhotoUrls,
    pub links: PhotoLinks,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub alt_description: Option<String>,
    pub user: UnsplashUser,
}

impl Photo {
    pub fn best_description(&self) -> Option<&str> {
        self.description
            .as_deref()
            .or(self.alt_description.as_deref())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PhotoUrls {
    pub raw: String,
    #[serde(default)]
    pub full: Option<String>,
    #[serde(default)]
    pub regular: Option<String>,
}

impl PhotoUrls {
    pub fn wallpaper_url(&self) -> &str {
        self.full
            .as_deref()
            .or(self.regular.as_deref())
            .unwrap_or(&self.raw)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PhotoLinks {
    pub html: String,
    pub download_location: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UnsplashUser {
    pub name: String,
}

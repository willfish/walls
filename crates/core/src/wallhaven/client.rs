use std::path::{Path, PathBuf};

use reqwest::Client;

use crate::config::{WallhavenCollection, WallhavenSearch};
use crate::quota::enforce_download_quota;

use super::types::{SearchResponse, Wallpaper, WallpaperResponse};

pub const DEFAULT_API_BASE: &str = "https://wallhaven.cc";

pub fn api_base() -> String {
    std::env::var("WALLHAVEN_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

pub struct WallhavenClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl WallhavenClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> anyhow::Result<Self> {
        let base = base_url.into();
        Ok(Self {
            http: Client::new(),
            base_url: base.trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        })
    }

    pub async fn search(
        &self,
        params: &WallhavenSearch,
        page: u32,
    ) -> anyhow::Result<SearchResponse> {
        let url = format!("{}/api/v1/search", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .query(&[
                ("q", params.q.as_str()),
                ("categories", params.categories.as_str()),
                ("purity", params.purity.as_str()),
                ("sorting", params.sorting.as_str()),
                ("order", params.order.as_str()),
                ("atleast", params.atleast.as_str()),
                ("page", &page.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn download_to_cache(
        &self,
        wp: &Wallpaper,
        cache_dir: &Path,
    ) -> anyhow::Result<PathBuf> {
        let ext = wp
            .path
            .rsplit('.')
            .next()
            .filter(|e| !e.is_empty())
            .unwrap_or("jpg");
        let dest = cache_dir.join(format!("wallhaven-{}.{}", wp.id, ext));
        if dest.exists() {
            return Ok(dest);
        }
        let bytes = self.http.get(&wp.path).send().await?.bytes().await?;
        tokio::fs::write(&dest, &bytes).await?;
        Ok(dest)
    }

    pub async fn download_to_cache_with_quota(
        &self,
        wp: &Wallpaper,
        cache_dir: &Path,
        download_dir: &Path,
        quota_mb: u64,
        quota_enabled: bool,
    ) -> anyhow::Result<PathBuf> {
        let dest = self.download_to_cache(wp, cache_dir).await?;
        if let Some(name) = dest.file_name() {
            let dl_path = download_dir.join(name);
            tokio::fs::copy(&dest, &dl_path).await.ok();
        }
        if quota_enabled {
            enforce_download_quota(download_dir, quota_mb)?;
        }
        Ok(dest)
    }

    pub async fn collection_wallpapers(
        &self,
        collection: &WallhavenCollection,
        page: u32,
    ) -> anyhow::Result<SearchResponse> {
        let url = format!(
            "{}/api/v1/collections/{}/{}",
            self.base_url, collection.username, collection.id
        );
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .query(&[("page", page.to_string())])
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn fetch_wallpaper(&self, id: &str) -> anyhow::Result<Wallpaper> {
        let url = format!("{}/api/v1/w/{}", self.base_url, id);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?
            .error_for_status()?;
        let body: WallpaperResponse = resp.json().await?;
        Ok(body.data)
    }
}

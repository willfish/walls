use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Client;

use crate::config::UnsplashSourceConfig;
use crate::downloads::{
    mirror_provider_cache_with_quota, response_bytes_limited, write_file_atomic,
    DEFAULT_MAX_PROVIDER_DOWNLOAD_BYTES,
};
use crate::provider_http;

use super::types::Photo;

pub const DEFAULT_API_BASE: &str = "https://api.unsplash.com";

pub fn api_base() -> String {
    std::env::var("UNSPLASH_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

pub struct UnsplashClient {
    http: Client,
    base_url: String,
    access_key: String,
    max_download_bytes: u64,
}

impl UnsplashClient {
    pub fn new(base_url: impl Into<String>, access_key: impl Into<String>) -> anyhow::Result<Self> {
        Self::new_with_timeouts(
            base_url,
            access_key,
            provider_http::DEFAULT_REQUEST_TIMEOUT,
            provider_http::DEFAULT_CONNECT_TIMEOUT,
        )
    }

    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        access_key: impl Into<String>,
        request_timeout: Duration,
        connect_timeout: Duration,
    ) -> anyhow::Result<Self> {
        Self::new_with_limits(
            base_url,
            access_key,
            request_timeout,
            connect_timeout,
            DEFAULT_MAX_PROVIDER_DOWNLOAD_BYTES,
        )
    }

    pub fn new_with_limits(
        base_url: impl Into<String>,
        access_key: impl Into<String>,
        request_timeout: Duration,
        connect_timeout: Duration,
        max_download_bytes: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            http: provider_http::client_with_timeouts(request_timeout, connect_timeout)?,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            access_key: access_key.into(),
            max_download_bytes,
        })
    }

    pub async fn random_photo(&self, source: &UnsplashSourceConfig) -> anyhow::Result<Photo> {
        let url = format!("{}/photos/random", self.base_url);
        let mut query = vec![("content_filter", "high"), ("count", "1")];
        if let Some(value) = source.query.as_deref() {
            query.push(("query", value));
        }
        if let Some(value) = source.collection.as_deref() {
            query.push(("collections", value));
        }
        if let Some(value) = source.user.as_deref() {
            query.push(("username", value));
        }
        if let Some(value) = source.topic.as_deref() {
            query.push(("topics", value));
        }
        if let Some(value) = source.orientation.as_deref() {
            query.push(("orientation", value));
        }

        let resp =
            provider_http::send_with_retries(|| self.authorized_get(&url).query(&query)).await?;
        let mut photos: Vec<Photo> = resp.json().await?;
        photos
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Unsplash random photo response was empty"))
    }

    pub async fn fetch_photo(&self, id: &str) -> anyhow::Result<Photo> {
        let url = format!("{}/photos/{id}", self.base_url);
        let resp = provider_http::send_with_retries(|| self.authorized_get(&url)).await?;
        Ok(resp.json().await?)
    }

    pub async fn download_to_cache_with_quota(
        &self,
        photo: &Photo,
        cache_dir: &Path,
        download_dir: &Path,
        quota_mb: u64,
        quota_enabled: bool,
    ) -> anyhow::Result<PathBuf> {
        let dest = self.download_to_cache(photo, cache_dir).await?;
        mirror_provider_cache_with_quota(&dest, download_dir, quota_mb, quota_enabled).await?;
        Ok(dest)
    }

    async fn download_to_cache(&self, photo: &Photo, cache_dir: &Path) -> anyhow::Result<PathBuf> {
        let ext = photo_extension(photo.urls.wallpaper_url()).unwrap_or_else(|| "jpg".into());
        let dest = cache_dir.join(format!("unsplash-{}.{}", photo.id, ext));
        if dest.exists() {
            return Ok(dest);
        }

        self.track_download(photo).await?;
        let response =
            provider_http::send_with_retries(|| self.http.get(photo.urls.wallpaper_url())).await?;
        let bytes = response_bytes_limited(response, self.max_download_bytes, "Unsplash").await?;
        write_file_atomic(&dest, &bytes).await?;
        Ok(dest)
    }

    async fn track_download(&self, photo: &Photo) -> anyhow::Result<()> {
        provider_http::send_with_retries(|| self.authorized_get(&photo.links.download_location))
            .await?;
        Ok(())
    }

    fn authorized_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Client-ID {}", self.access_key))
    }
}

fn photo_extension(url: &str) -> Option<String> {
    let path = reqwest::Url::parse(url).ok()?;
    path.path()
        .rsplit('.')
        .next()
        .filter(|extension| !extension.is_empty())
        .map(str::to_string)
}

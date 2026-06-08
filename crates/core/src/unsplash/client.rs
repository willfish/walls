use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context};
use reqwest::{Client, Response, StatusCode};

use crate::config::UnsplashSourceConfig;
use crate::downloads::{copy_file_atomic, write_file_atomic};
use crate::quota::enforce_download_quota;

use super::types::Photo;

pub const DEFAULT_API_BASE: &str = "https://api.unsplash.com";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 3;
const RETRY_BACKOFF_BASE_MS: u64 = 100;

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
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_CONNECT_TIMEOUT,
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
            DEFAULT_MAX_DOWNLOAD_BYTES,
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
            http: Client::builder()
                .connect_timeout(connect_timeout)
                .timeout(request_timeout)
                .build()?,
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

        let resp = self
            .send_with_retries(|| self.authorized_get(&url).query(&query))
            .await?;
        let mut photos: Vec<Photo> = resp.json().await?;
        photos
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Unsplash random photo response was empty"))
    }

    pub async fn fetch_photo(&self, id: &str) -> anyhow::Result<Photo> {
        let url = format!("{}/photos/{id}", self.base_url);
        let resp = self.send_with_retries(|| self.authorized_get(&url)).await?;
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
        if let Some(name) = dest.file_name() {
            let dl_path = download_dir.join(name);
            copy_file_atomic(&dest, &dl_path).await.ok();
        }
        if quota_enabled {
            enforce_download_quota(download_dir, quota_mb)?;
        }
        Ok(dest)
    }

    async fn download_to_cache(&self, photo: &Photo, cache_dir: &Path) -> anyhow::Result<PathBuf> {
        let ext = photo_extension(photo.urls.wallpaper_url()).unwrap_or_else(|| "jpg".into());
        let dest = cache_dir.join(format!("unsplash-{}.{}", photo.id, ext));
        if dest.exists() {
            return Ok(dest);
        }

        self.track_download(photo).await?;
        let response = self
            .send_with_retries(|| self.http.get(photo.urls.wallpaper_url()))
            .await?;
        let bytes = pipe_limited_body(response, self.max_download_bytes).await?;
        write_file_atomic(&dest, &bytes).await?;
        Ok(dest)
    }

    async fn track_download(&self, photo: &Photo) -> anyhow::Result<()> {
        self.send_with_retries(|| self.authorized_get(&photo.links.download_location))
            .await?;
        Ok(())
    }

    fn authorized_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header("Authorization", format!("Client-ID {}", self.access_key))
    }

    async fn send_with_retries(
        &self,
        mut build_request: impl FnMut() -> reqwest::RequestBuilder,
    ) -> anyhow::Result<Response> {
        let mut attempt = 1;
        loop {
            let resp = build_request().send().await?;
            let status = resp.status();
            if !is_transient_status(status) || attempt == MAX_ATTEMPTS {
                return Ok(resp.error_for_status()?);
            }

            tokio::time::sleep(backoff_delay(attempt)).await;
            attempt += 1;
        }
    }
}

async fn pipe_limited_body(mut response: Response, max_bytes: u64) -> anyhow::Result<Vec<u8>> {
    if let Some(content_length) = response.content_length() {
        ensure!(
            content_length <= max_bytes,
            "Unsplash download size {content_length} bytes exceeds limit of {max_bytes} bytes"
        );
    }

    let mut total = 0_u64;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let chunk_len = u64::try_from(chunk.len())?;
        total = total
            .checked_add(chunk_len)
            .context("Unsplash download size overflowed while reading response")?;
        ensure!(
            total <= max_bytes,
            "Unsplash download exceeded limit of {max_bytes} bytes while reading response"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn photo_extension(url: &str) -> Option<String> {
    let path = reqwest::Url::parse(url).ok()?;
    path.path()
        .rsplit('.')
        .next()
        .filter(|extension| !extension.is_empty())
        .map(str::to_string)
}

fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BACKOFF_BASE_MS * u64::from(attempt))
}

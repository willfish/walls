use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context};
use reqwest::{Client, Response, StatusCode};

use crate::config::{WallhavenCollection, WallhavenSearch};
use crate::quota::enforce_download_quota;

use super::types::{SearchResponse, Wallpaper, WallpaperResponse};

pub const DEFAULT_API_BASE: &str = "https://wallhaven.cc";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 3;
const RETRY_BACKOFF_BASE_MS: u64 = 100;

pub fn api_base() -> String {
    std::env::var("WALLHAVEN_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.to_string())
}

pub struct WallhavenClient {
    http: Client,
    base_url: String,
    api_key: String,
    max_download_bytes: u64,
}

impl WallhavenClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> anyhow::Result<Self> {
        Self::new_with_timeouts(
            base_url,
            api_key,
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_CONNECT_TIMEOUT,
        )
    }

    pub fn new_with_timeouts(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        request_timeout: Duration,
        connect_timeout: Duration,
    ) -> anyhow::Result<Self> {
        Self::new_with_limits(
            base_url,
            api_key,
            request_timeout,
            connect_timeout,
            DEFAULT_MAX_DOWNLOAD_BYTES,
        )
    }

    pub fn new_with_limits(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        request_timeout: Duration,
        connect_timeout: Duration,
        max_download_bytes: u64,
    ) -> anyhow::Result<Self> {
        let base = base_url.into();
        Ok(Self {
            http: Client::builder()
                .connect_timeout(connect_timeout)
                .timeout(request_timeout)
                .build()?,
            base_url: base.trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            max_download_bytes,
        })
    }

    pub async fn search(
        &self,
        params: &WallhavenSearch,
        page: u32,
    ) -> anyhow::Result<SearchResponse> {
        let url = format!("{}/api/v1/search", self.base_url);
        let page = page.to_string();
        let resp = self
            .send_with_retries(|| {
                self.http
                    .get(&url)
                    .header("X-API-Key", &self.api_key)
                    .query(&[
                        ("q", params.q.as_str()),
                        ("categories", params.categories.as_str()),
                        ("purity", params.purity.as_str()),
                        ("sorting", params.sorting.as_str()),
                        ("order", params.order.as_str()),
                        ("atleast", params.atleast.as_str()),
                        ("page", page.as_str()),
                    ])
            })
            .await?;
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
        let response = self.send_with_retries(|| self.http.get(&wp.path)).await?;
        let bytes = pipe_limited_body(response, self.max_download_bytes).await?;
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
        let page = page.to_string();
        let resp = self
            .send_with_retries(|| {
                self.http
                    .get(&url)
                    .header("X-API-Key", &self.api_key)
                    .query(&[("page", page.as_str())])
            })
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn fetch_wallpaper(&self, id: &str) -> anyhow::Result<Wallpaper> {
        let url = format!("{}/api/v1/w/{}", self.base_url, id);
        let resp = self
            .send_with_retries(|| self.http.get(&url).header("X-API-Key", &self.api_key))
            .await?;
        let body: WallpaperResponse = resp.json().await?;
        Ok(body.data)
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
            "Wallhaven download size {content_length} bytes exceeds limit of {max_bytes} bytes"
        );
    }

    let mut total = 0_u64;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let chunk_len = u64::try_from(chunk.len())?;
        total = total
            .checked_add(chunk_len)
            .context("Wallhaven download size overflowed while reading response")?;
        ensure!(
            total <= max_bytes,
            "Wallhaven download exceeded limit of {max_bytes} bytes while reading response"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BACKOFF_BASE_MS * u64::from(attempt))
}

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context};
use reqwest::{Client, Response};

use crate::config::{wallhaven_effective_query, WallhavenCollection, WallhavenSearch};
use crate::downloads::{copy_file_atomic, write_file_atomic};
use crate::provider_http;
use crate::quota::enforce_download_quota;

use super::types::{SearchResponse, Wallpaper, WallpaperResponse};

pub const DEFAULT_API_BASE: &str = "https://wallhaven.cc";
const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;

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
            provider_http::DEFAULT_REQUEST_TIMEOUT,
            provider_http::DEFAULT_CONNECT_TIMEOUT,
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
            http: provider_http::client_with_timeouts(request_timeout, connect_timeout)?,
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
        let purity = purity_for_request(&params.purity, &self.api_key);
        let q = wallhaven_effective_query(params);
        let mut query = vec![
            ("q", q.as_str()),
            ("categories", params.categories.as_str()),
            ("purity", purity.as_str()),
            ("sorting", params.sorting.as_str()),
            ("order", params.order.as_str()),
            ("page", page.as_str()),
        ];
        if !params.atleast.trim().is_empty() {
            query.push(("atleast", params.atleast.as_str()));
        }
        if !params.ratios.trim().is_empty() {
            query.push(("ratios", params.ratios.as_str()));
        }
        let resp = provider_http::send_with_retries(|| {
            self.http
                .get(&url)
                .header("X-API-Key", &self.api_key)
                .query(&query)
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
        let response = provider_http::send_with_retries(|| self.http.get(&wp.path)).await?;
        let bytes = pipe_limited_body(response, self.max_download_bytes).await?;
        write_file_atomic(&dest, &bytes).await?;
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
            copy_file_atomic(&dest, &dl_path).await.ok();
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
        let resp = provider_http::send_with_retries(|| {
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
        let resp = provider_http::send_with_retries(|| {
            self.http.get(&url).header("X-API-Key", &self.api_key)
        })
        .await?;
        let body: WallpaperResponse = resp.json().await?;
        Ok(body.data)
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

/// Anonymous Wallhaven access ignores NSFW purity; strip that bit when no API key is set.
fn purity_for_request(purity: &str, api_key: &str) -> String {
    if !api_key.trim().is_empty() {
        return purity.to_string();
    }
    let mut chars: Vec<char> = purity.chars().collect();
    if chars.len() < 3 {
        chars.resize(3, '0');
    }
    chars[2] = '0';
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purity_for_request_strips_nsfw_without_api_key() {
        assert_eq!(purity_for_request("111", ""), "110");
        assert_eq!(purity_for_request("101", ""), "100");
        assert_eq!(purity_for_request("111", "key"), "111");
    }
}

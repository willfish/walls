use std::path::{Path, PathBuf};

use anyhow::Context;
use rand::RngExt;
use reqwest::Client;

use crate::apply::ApplyTrigger;
use crate::config::{SourceEntry, SourceKind};
use crate::ctx::WallsCtx;
use crate::downloads::write_file_atomic;
use crate::provider_http;
use crate::providers::{provider_for_source, ProviderDescriptor};
use crate::state::CurrentWallMetadata;

pub(crate) fn first_enabled_source(
    sources: &[SourceEntry],
    source_kind: SourceKind,
    internet_required: bool,
    internet_enabled: bool,
) -> Option<&SourceEntry> {
    if internet_required && !internet_enabled {
        return None;
    }
    sources
        .iter()
        .find(|source| source.enabled && SourceKind::parse(&source.source_type) == source_kind)
}

pub(crate) fn provider_for(entry: &SourceEntry) -> ProviderDescriptor {
    provider_for_source(entry)
}

pub(crate) async fn download_bytes(
    client: &Client,
    url: &str,
    provider: &ProviderDescriptor,
    operation: &'static str,
) -> anyhow::Result<Vec<u8>> {
    provider_http::send_with_retries(|| client.get(url))
        .await
        .with_context(|| provider.failure_scope(operation).to_string())?
        .bytes()
        .await
        .with_context(|| provider.failure_scope(operation).to_string())
        .map(|b| b.to_vec())
}

pub(crate) async fn write_cache_and_apply(
    ctx: &mut WallsCtx,
    cache_file: &str,
    bytes: &[u8],
    source_id: impl Into<String>,
    metadata: CurrentWallMetadata,
) -> anyhow::Result<PathBuf> {
    let dest = ctx.paths.cache_dir.join(cache_file);
    write_file_atomic(&dest, bytes).await?;
    ctx.apply_file_inner_with_metadata(
        &dest,
        ApplyTrigger::Auto,
        Some(source_id.into()),
        metadata,
        true,
    )?;
    Ok(dest)
}

pub(crate) fn api_base(env_key: &str, default: &str) -> String {
    std::env::var(env_key)
        .unwrap_or_else(|_| default.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn http_client() -> anyhow::Result<Client> {
    provider_http::client()
}

pub(crate) fn http_client_with_headers(
    headers: reqwest::header::HeaderMap,
) -> anyhow::Result<Client> {
    provider_http::client_with_headers(headers)
}

pub(crate) async fn send_with_retries(
    build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> anyhow::Result<reqwest::Response> {
    provider_http::send_with_retries(build_request).await
}

pub(crate) fn reddit_user_agent() -> &'static str {
    "walls-wallpaper-manager/0.8"
}

pub(crate) fn is_probably_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let ext = std::path::Path::new(&lower).extension();
    ext.is_some_and(|ext| {
        ext.eq_ignore_ascii_case("jpg")
            || ext.eq_ignore_ascii_case("jpeg")
            || ext.eq_ignore_ascii_case("png")
            || ext.eq_ignore_ascii_case("webp")
    }) || lower.contains("i.redd.it")
        || lower.contains("preview.redd.it")
}

pub(crate) fn fix_imgur_url(url: &str) -> String {
    if regex::Regex::new(r"^https?://imgur\.com/\w+$")
        .ok()
        .and_then(|re| re.is_match(url).then_some(()))
        .is_some()
    {
        return url.replacen("://", "://i.", 1) + ".jpg";
    }
    url.to_string()
}

pub(crate) fn pick_random<T>(items: &[T]) -> Option<&T> {
    if items.is_empty() {
        return None;
    }
    let index = rand::rng().random_range(0..items.len());
    items.get(index)
}

pub(crate) fn expand_dir(path: &str) -> PathBuf {
    crate::paths::expand_home(path)
}

pub(crate) fn pick_random_image_in_dir(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let mut images = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_image_path(path) {
            images.push(path.to_path_buf());
        }
    }
    images.sort();
    pick_random(&images).cloned()
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "avif" | "bmp" | "gif"
            )
        })
}

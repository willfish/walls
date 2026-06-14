use std::path::PathBuf;

use crate::config::SourceKind;
use crate::ctx::WallsCtx;
use crate::inline_providers::common::{
    api_base, download_bytes, enabled_sources, pick_random, provider_for, write_cache_and_apply,
};
use crate::state::CurrentWallMetadata;
use anyhow::Context;

pub async fn try_pixabay(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let sources = enabled_sources(
        &ctx.config.sources,
        SourceKind::Pixabay,
        true,
        ctx.config.change.internet_enabled,
    );
    if sources.is_empty() {
        return Ok(None);
    }
    let mut last_error = None;
    for source in sources {
        match try_pixabay_source(ctx, &source).await {
            Ok(Some(path)) => return Ok(Some(path)),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    Ok(None)
}

async fn try_pixabay_source(
    ctx: &mut WallsCtx,
    src: &crate::config::SourceEntry,
) -> anyhow::Result<Option<PathBuf>> {
    let provider = provider_for(src);
    let api_key = src
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("pixabay source missing api_key"))?;
    let query = src.query.as_deref().unwrap_or("nature");
    let base = api_base("PIXABAY_API_BASE", "https://pixabay.com");
    let url = format!(
        "{base}/api/?key={api_key}&q={query}&image_type=photo&orientation=horizontal&per_page=20&safesearch=true"
    );

    let client = crate::inline_providers::common::http_client()?;
    let payload: serde_json::Value =
        crate::inline_providers::common::send_with_retries(|| client.get(&url))
            .await
            .with_context(|| provider.failure_scope("pixabay search fetch").to_string())?
            .json()
            .await
            .with_context(|| provider.failure_scope("pixabay search parse").to_string())?;

    let hits = payload
        .get("hits")
        .and_then(|v| v.as_array())
        .filter(|hits| !hits.is_empty())
        .ok_or_else(|| anyhow::anyhow!("pixabay search returned no hits"))?;

    let hit = pick_random(hits).ok_or_else(|| anyhow::anyhow!("pixabay hits empty"))?;
    let image_url = hit
        .get("largeImageURL")
        .or_else(|| hit.get("webformatURL"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("pixabay hit missing image url"))?;

    let bytes = download_bytes(&client, image_url, &provider, "pixabay image download").await?;
    let dest = write_cache_and_apply(
        ctx,
        "pixabay-fetch.jpg",
        &bytes,
        format!("pixabay:{query}"),
        CurrentWallMetadata {
            provider: Some("pixabay".into()),
            source_url: hit
                .get("pageURL")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            author: hit.get("user").and_then(|v| v.as_str()).map(str::to_string),
            description: hit.get("tags").and_then(|v| v.as_str()).map(str::to_string),
        },
    )
    .await?;

    Ok(Some(dest))
}

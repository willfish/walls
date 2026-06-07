use std::path::PathBuf;

use anyhow::Context;
use reqwest::Client;

use crate::ctx::WallsCtx;
use crate::inline_providers::common::{
    api_base, download_bytes, first_enabled_source, provider_for, write_cache_and_apply,
};
use crate::state::CurrentWallMetadata;

pub async fn try_apod(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        "apod",
        true,
        ctx.config.change.internet_enabled,
    ) else {
        return Ok(None);
    };

    let provider = provider_for(src);
    let api_key = src
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or("DEMO_KEY");
    let base = api_base("NASA_API_BASE", "https://api.nasa.gov");
    let url = format!("{base}/planetary/apod?api_key={api_key}");

    let client = Client::new();
    let payload: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .with_context(|| provider.failure_scope("apod metadata fetch").to_string())?
        .json()
        .await
        .with_context(|| provider.failure_scope("apod metadata parse").to_string())?;

    if payload.get("media_type").and_then(|v| v.as_str()) == Some("video") {
        tracing::info!("apod: today's entry is a video, skipping");
        return Ok(None);
    }

    let image_url = payload
        .get("hdurl")
        .or_else(|| payload.get("url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("apod response missing image url"))?;

    let bytes = download_bytes(&client, image_url, &provider, "apod image download").await?;
    let dest = write_cache_and_apply(
        ctx,
        "apod-daily.jpg",
        &bytes,
        "apod-daily",
        CurrentWallMetadata {
            provider: Some("apod".into()),
            source_url: payload
                .get("url")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            author: Some("NASA APOD".into()),
            description: payload
                .get("title")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        },
    )
    .await?;

    Ok(Some(dest))
}

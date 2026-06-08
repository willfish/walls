use std::path::PathBuf;

use anyhow::Context;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;

use crate::ctx::WallsCtx;
use crate::inline_providers::common::{
    download_bytes, first_enabled_source, provider_for, write_cache_and_apply,
};
use crate::state::CurrentWallMetadata;

pub async fn try_immich(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        "immich",
        true,
        ctx.config.change.internet_enabled,
    ) else {
        return Ok(None);
    };

    let provider = provider_for(src);
    let server = src
        .url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("immich source missing url"))?;
    let api_key = src
        .api_key
        .as_deref()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("immich source missing api_key"))?;

    let base = server.trim_end_matches('/');
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(api_key).context("invalid immich api key header")?,
    );

    let client = Client::builder().default_headers(headers).build()?;
    let random_url = format!("{base}/api/search/random?type=IMAGE");
    let payload: serde_json::Value = client
        .get(&random_url)
        .send()
        .await
        .with_context(|| provider.failure_scope("immich random fetch").to_string())?
        .json()
        .await
        .with_context(|| provider.failure_scope("immich random parse").to_string())?;

    let asset = payload
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&payload);

    let asset_id = json_id(asset.get("id").unwrap_or(&serde_json::Value::Null))
        .or_else(|| asset.pointer("/asset/id").and_then(json_id))
        .ok_or_else(|| anyhow::anyhow!("immich random response missing asset id"))?;

    let download_url = format!("{base}/api/assets/{asset_id}/original");
    let bytes = download_bytes(&client, &download_url, &provider, "immich image download").await?;
    let dest = write_cache_and_apply(
        ctx,
        "immich-fetch.jpg",
        &bytes,
        format!("immich:{asset_id}"),
        CurrentWallMetadata {
            provider: Some("immich".into()),
            source_url: Some(download_url),
            author: None,
            description: asset
                .get("originalFileName")
                .and_then(|v| v.as_str())
                .or(src.label.as_deref())
                .map(str::to_string),
        },
    )
    .await?;

    Ok(Some(dest))
}

fn json_id(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|n| n.to_string()))
}

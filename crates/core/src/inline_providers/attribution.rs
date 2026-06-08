use std::path::PathBuf;

use crate::apply::ApplyTrigger;
use crate::ctx::WallsCtx;
use crate::downloads::write_file_atomic;
use crate::inline_providers::common::{download_bytes, first_enabled_source, provider_for};
use crate::state::CurrentWallMetadata;

pub async fn try_attribution(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        "attribution",
        true,
        ctx.config.change.internet_enabled,
    ) else {
        return Ok(None);
    };

    let provider = provider_for(src);
    let image_url = src
        .url
        .as_deref()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("attribution source missing url"))?;

    let client = reqwest::Client::new();
    let bytes = download_bytes(&client, image_url, &provider, "attribution image download").await?;

    let dest = ctx.paths.cache_dir.join("attribution-fetch.jpg");
    write_file_atomic(&dest, &bytes).await?;

    let label = src.label.clone().unwrap_or_else(|| "attribution".into());
    ctx.apply_file_inner_with_metadata(
        &dest,
        ApplyTrigger::Auto,
        Some(label.clone()),
        CurrentWallMetadata {
            provider: Some("attribution".into()),
            source_url: Some(image_url.to_string()),
            author: None,
            description: Some(label),
        },
        true,
    )?;

    Ok(Some(dest))
}

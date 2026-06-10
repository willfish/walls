use std::path::PathBuf;

use crate::apply::ApplyTrigger;
use crate::config::SourceKind;
use crate::ctx::WallsCtx;
use crate::downloads::write_file_atomic;
use crate::inline_providers::common::{download_bytes, first_enabled_source, provider_for};
use crate::state::CurrentWallMetadata;

pub async fn try_attribution(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        SourceKind::Attribution,
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

    let client = crate::inline_providers::common::http_client()?;
    let bytes = download_bytes(&client, image_url, &provider, "attribution image download").await?;

    let dest = ctx.paths.cache_dir.join("attribution-fetch.jpg");
    write_file_atomic(&dest, &bytes).await?;

    let label = src.label.clone().unwrap_or_else(|| "attribution".into());
    let description = src
        .source
        .clone()
        .filter(|source| !source.trim().is_empty())
        .unwrap_or_else(|| label.clone());
    ctx.apply_file_inner_with_metadata(
        &dest,
        ApplyTrigger::Auto,
        Some(label.clone()),
        CurrentWallMetadata {
            provider: Some("attribution".into()),
            source_url: Some(image_url.to_string()),
            author: src.author.clone(),
            description: Some(description),
        },
        true,
    )?;

    Ok(Some(dest))
}

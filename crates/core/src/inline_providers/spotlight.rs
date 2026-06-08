use std::path::PathBuf;

use crate::apply::ApplyTrigger;
use crate::config::SourceKind;
use crate::ctx::WallsCtx;
use crate::inline_providers::common::{expand_dir, first_enabled_source, pick_random_image_in_dir};

/// Windows Spotlight cache or any local folder configured via `path` / `url`.
pub async fn try_spotlight(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        SourceKind::Spotlight,
        false,
        ctx.config.change.internet_enabled,
    ) else {
        return Ok(None);
    };

    let dir = src
        .path
        .as_deref()
        .or(src.url.as_deref())
        .map(expand_dir)
        .filter(|path| path.is_dir());

    let Some(dir) = dir else {
        tracing::info!(
            "spotlight: no readable folder configured (set path or url to a Spotlight assets directory)"
        );
        return Ok(None);
    };

    let Some(image) = pick_random_image_in_dir(&dir) else {
        tracing::info!("spotlight: no images found in {}", dir.display());
        return Ok(None);
    };

    let label = src.label.clone().unwrap_or_else(|| "spotlight".into());
    ctx.apply_file_inner(&image, ApplyTrigger::Auto, Some(label), true)?;
    Ok(Some(image))
}

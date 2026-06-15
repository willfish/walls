use std::path::PathBuf;

use anyhow::Context;

use super::WallsCtx;
use crate::apply::ApplyTrigger;
use crate::providers::{
    ProviderDescriptor, ProviderFailureKind, ProviderNoCandidateReason, ProviderOperation,
    ProviderStatus,
};

pub(super) async fn apply_wallhaven_queue(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let provider = crate::providers::wallhaven_provider(&ctx.config, &ctx.secrets);
    let client = crate::wallhaven::WallhavenClient::new(
        crate::wallhaven::api_base(),
        &ctx.secrets.wallhaven_api_key,
    )?;
    if let Some(path) = apply_wallhaven_queue_head(ctx, &client, &provider).await? {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::AdvanceNext)
                .applied(None),
        );
        return Ok(Some(path));
    }

    if !provider.enabled {
        let reason = if ctx.config.change.internet_enabled {
            ProviderNoCandidateReason::Disabled
        } else {
            ProviderNoCandidateReason::OfflineDisabled
        };
        let status = if reason == ProviderNoCandidateReason::OfflineDisabled {
            ProviderStatus::OfflineDisabled
        } else {
            ProviderStatus::Disabled
        };
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::QueueRefill)
                .with_status(status)
                .skipped(reason)
                .with_fallback("bing"),
        );
        return Ok(None);
    }

    match crate::wallhaven::refill_wallhaven_cache(&client, &ctx.config, &mut ctx.state).await {
        Ok(()) => ctx.save_state()?,
        Err(error) => {
            tracing::warn!(error = %error, "wallhaven: queue refill failed, trying next source");
            ctx.record_provider_attempt(
                provider
                    .attempt(ProviderOperation::QueueRefill)
                    .failed(ProviderFailureKind::Unknown, None, Some(error.to_string()))
                    .with_fallback("bing"),
            );
        }
    }

    let applied = apply_wallhaven_queue_head(ctx, &client, &provider).await?;
    if applied.is_some() {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::AdvanceNext)
                .applied(None),
        );
    } else if !ctx.provider_status_report.attempted_provider(&provider.id) {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::QueueRefill)
                .no_candidates(ProviderNoCandidateReason::QueueEmpty, Some(0))
                .with_fallback("bing"),
        );
    }
    Ok(applied)
}

async fn apply_wallhaven_queue_head(
    ctx: &mut WallsCtx,
    client: &crate::wallhaven::WallhavenClient,
    provider: &ProviderDescriptor,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(id) = ctx.state.cache_queue.first().cloned() else {
        return Ok(None);
    };
    if crate::unsplash::queue_photo_id(&id).is_some() {
        return Ok(None);
    }
    let path =
        if let Some(path) = crate::wallhaven::cached_wallpaper_path(&ctx.paths.cache_dir, &id) {
            path
        } else {
            let wallpaper = client
                .fetch_wallpaper(&id)
                .await
                .with_context(|| provider.failure_scope("metadata fetch").to_string())?;
            client
                .download_to_cache_with_quota(
                    &wallpaper,
                    &ctx.paths.cache_dir,
                    &ctx.paths.download_dir,
                    ctx.config.quota.size_mb,
                    ctx.config.quota.enabled,
                )
                .await
                .with_context(|| provider.failure_scope("download").to_string())?
        };
    ctx.state.cache_queue.remove(0);
    ctx.apply_file_inner(&path, ApplyTrigger::Auto, Some(id), true)?;
    Ok(Some(path))
}

pub(super) async fn apply_unsplash_queue(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let provider = crate::providers::unsplash_provider(&ctx.config, &ctx.secrets);
    if !provider.enabled {
        let (status, reason) = unsplash_unavailable_reason(ctx);
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::QueueRefill)
                .with_status(status)
                .skipped(reason)
                .with_fallback("wallhaven"),
        );
        return Ok(None);
    }

    let client = unsplash_client(ctx)?;
    if let Some(path) = apply_unsplash_queue_head(ctx, &client, &provider).await? {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::AdvanceNext)
                .applied(None),
        );
        return Ok(Some(path));
    }

    match crate::unsplash::refill_unsplash_cache(&client, &ctx.config, &mut ctx.state).await {
        Ok(()) => ctx.save_state()?,
        Err(error) => {
            tracing::warn!(error = %error, "unsplash: queue refill failed, trying next source");
            ctx.record_provider_attempt(
                provider
                    .attempt(ProviderOperation::QueueRefill)
                    .failed(ProviderFailureKind::Unknown, None, Some(error.to_string()))
                    .with_fallback("wallhaven"),
            );
        }
    }

    let applied = apply_unsplash_queue_head(ctx, &client, &provider).await?;
    if applied.is_some() {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::AdvanceNext)
                .applied(None),
        );
    } else if !ctx.provider_status_report.attempted_provider(&provider.id) {
        ctx.record_provider_attempt(
            provider
                .attempt(ProviderOperation::QueueRefill)
                .no_candidates(ProviderNoCandidateReason::QueueEmpty, Some(0))
                .with_fallback("wallhaven"),
        );
    }
    Ok(applied)
}

fn unsplash_unavailable_reason(ctx: &WallsCtx) -> (ProviderStatus, ProviderNoCandidateReason) {
    if !ctx.config.change.internet_enabled {
        return (
            ProviderStatus::OfflineDisabled,
            ProviderNoCandidateReason::OfflineDisabled,
        );
    }
    if ctx.secrets.unsplash_access_key.is_empty() {
        return (
            ProviderStatus::CredentialMissing,
            ProviderNoCandidateReason::CredentialMissing,
        );
    }
    (
        ProviderStatus::Disabled,
        ProviderNoCandidateReason::NoEnabledSource,
    )
}

fn unsplash_client(ctx: &WallsCtx) -> anyhow::Result<crate::unsplash::UnsplashClient> {
    crate::unsplash::UnsplashClient::new(
        crate::unsplash::api_base(),
        &ctx.secrets.unsplash_access_key,
    )
}

async fn apply_unsplash_queue_head(
    ctx: &mut WallsCtx,
    client: &crate::unsplash::UnsplashClient,
    provider: &ProviderDescriptor,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(queue_item) = ctx.state.cache_queue.first().cloned() else {
        return Ok(None);
    };
    let Some(photo_id) = crate::unsplash::queue_photo_id(&queue_item) else {
        return Ok(None);
    };

    let photo = client
        .fetch_photo(photo_id)
        .await
        .with_context(|| provider.failure_scope("metadata fetch").to_string())?;
    let path =
        if let Some(path) = crate::unsplash::cached_photo_path(&ctx.paths.cache_dir, photo_id) {
            path
        } else {
            client
                .download_to_cache_with_quota(
                    &photo,
                    &ctx.paths.cache_dir,
                    &ctx.paths.download_dir,
                    ctx.config.quota.size_mb,
                    ctx.config.quota.enabled,
                )
                .await
                .with_context(|| provider.failure_scope("download").to_string())?
        };

    ctx.state.cache_queue.remove(0);
    let description = photo.best_description().map(str::to_string);
    ctx.apply_file_inner_with_metadata(
        &path,
        ApplyTrigger::Auto,
        None,
        crate::state::CurrentWallMetadata {
            provider: Some("unsplash".into()),
            source_url: Some(photo.links.html),
            author: Some(photo.user.name),
            description,
        },
        true,
    )?;
    Ok(Some(path))
}

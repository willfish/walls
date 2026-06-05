use anyhow::Context;

use crate::config::{Config, SourceEntry, UnsplashSourceConfig};
use crate::state::State;

use super::cache::queue_id;
use super::client::UnsplashClient;

pub fn enabled_unsplash_sources(sources: &[SourceEntry]) -> impl Iterator<Item = &SourceEntry> {
    sources
        .iter()
        .filter(|source| source.enabled && source.source_type == "unsplash")
}

pub async fn refill_unsplash_cache(
    client: &UnsplashClient,
    config: &Config,
    state: &mut State,
) -> anyhow::Result<()> {
    let threshold = config.selection.refetch_when_cache_below;
    if state.cache_queue.len() >= threshold || !config.change.internet_enabled {
        return Ok(());
    }

    for source in enabled_unsplash_sources(&config.sources) {
        if state.cache_queue.len() >= threshold {
            break;
        }
        let source_label = source.label.as_deref().unwrap_or("unsplash");
        let source_config = UnsplashSourceConfig::from_source(source)
            .with_context(|| format!("unsplash source {source_label}: config"))?;
        let photo = client
            .random_photo(&source_config)
            .await
            .with_context(|| format!("unsplash source {source_label}: random photo"))?;
        let id = queue_id(&photo.id);
        if !state.cache_queue.contains(&id) && !state.history.contains(&id) {
            state.cache_queue.push(id);
        }
    }

    Ok(())
}

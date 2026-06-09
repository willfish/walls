use crate::config::{
    source_wallhaven_prefer, source_wallhaven_search, Config, SourceKind, WallhavenPrefer,
};
use crate::state::State;

use super::client::WallhavenClient;

fn push_ids(state: &mut State, ids: impl IntoIterator<Item = String>) {
    for id in ids {
        if !state.cache_queue.contains(&id) && !state.history.contains(&id) {
            state.cache_queue.push(id);
        }
    }
}

pub async fn refill_wallhaven_cache(
    client: &WallhavenClient,
    config: &Config,
    state: &mut State,
) -> anyhow::Result<()> {
    let threshold = config.selection.refetch_when_cache_below;
    if state.cache_queue.len() >= threshold {
        return Ok(());
    }
    if !config.change.internet_enabled {
        return Ok(());
    }

    for (index, source) in config.sources.iter().enumerate().filter(|(_, source)| {
        source.enabled && SourceKind::parse(&source.source_type) == SourceKind::Wallhaven
    }) {
        if state.cache_queue.len() >= threshold {
            return Ok(());
        }
        let prefer = source_wallhaven_prefer(source);
        if matches!(
            prefer,
            WallhavenPrefer::CollectionsOnly | WallhavenPrefer::CollectionsThenSearch
        ) {
            for coll in &source.collections {
                if state.cache_queue.len() >= threshold {
                    return Ok(());
                }
                let key = format!("{index}:{}:{}", coll.username, coll.id);
                let page = state
                    .wallhaven
                    .collection_pages
                    .get(&key)
                    .copied()
                    .unwrap_or(1)
                    .max(1);
                let resp = client.collection_wallpapers(coll, page).await?;
                push_ids(state, resp.data.into_iter().map(|wp| wp.id));
                state.wallhaven.collection_pages.insert(
                    key,
                    if resp.meta.current_page < resp.meta.last_page {
                        resp.meta.current_page + 1
                    } else {
                        1
                    },
                );
            }
        }

        if state.cache_queue.len() >= threshold || prefer == WallhavenPrefer::CollectionsOnly {
            continue;
        }

        let Some(query) = source
            .query
            .as_deref()
            .filter(|query| !query.trim().is_empty())
        else {
            continue;
        };
        let key = format!(
            "{}:{}",
            index,
            source
                .label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(query)
        );
        let page = state
            .wallhaven
            .source_search_pages
            .get(&key)
            .copied()
            .unwrap_or(1)
            .max(1);
        let search = source_wallhaven_search(source);
        let resp = client.search(&search, page).await?;
        push_ids(state, resp.data.into_iter().map(|wp| wp.id));
        state.wallhaven.source_search_pages.insert(
            key,
            if resp.meta.current_page < resp.meta.last_page {
                resp.meta.current_page + 1
            } else {
                1
            },
        );
    }

    Ok(())
}

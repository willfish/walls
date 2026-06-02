use crate::config::{Config, WallhavenPrefer};
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

    let prefer = config.wallhaven.prefer;
    if matches!(
        prefer,
        WallhavenPrefer::CollectionsOnly | WallhavenPrefer::CollectionsThenSearch
    ) {
        for coll in &config.wallhaven.collections {
            if state.cache_queue.len() >= threshold {
                break;
            }
            let key = format!("{}:{}", coll.username, coll.id);
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
        return Ok(());
    }

    let page = state.wallhaven.search_page.max(1);
    let resp = client.search(&config.wallhaven.search, page).await?;
    push_ids(state, resp.data.into_iter().map(|wp| wp.id));
    state.wallhaven.search_page = if resp.meta.current_page < resp.meta.last_page {
        resp.meta.current_page + 1
    } else {
        1
    };
    Ok(())
}

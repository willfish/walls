use std::collections::HashMap;

use walls_core::config::{
    default_wallhaven_source, source_wallhaven_prefer, source_wallhaven_search, Config, Secrets,
    SourceEntry, WallhavenCollection, WallhavenPrefer, WallhavenSearch,
};

use super::App;

pub(super) fn bit_at(s: &str, idx: usize, default: bool) -> bool {
    s.chars().nth(idx).map(|c| c == '1').unwrap_or(default)
}

fn bits_from_bools(a: bool, b: bool, c: bool) -> String {
    format!("{}{}{}", u8::from(a), u8::from(b), u8::from(c))
}

pub(super) fn api_key_present(secrets: &Secrets) -> bool {
    !secrets.wallhaven_api_key.trim().is_empty()
}

pub(super) fn block_draft(config: &Config, api_key_present: bool) -> HashMap<String, String> {
    let source = first_source(config);
    let search = source.map(source_wallhaven_search).unwrap_or_default();
    let mut vals = HashMap::new();
    vals.insert(
        "enabled".into(),
        source.is_some_and(|source| source.enabled).to_string(),
    );
    vals.insert(
        "prefer".into(),
        prefer_label(source.map(source_wallhaven_prefer).unwrap_or_default()),
    );
    vals.extend(search_draft(&search, api_key_present));
    vals
}

pub(super) fn search_draft(
    search: &WallhavenSearch,
    api_key_present: bool,
) -> HashMap<String, String> {
    let mut vals = HashMap::new();
    vals.insert("search_q".into(), search.q.clone());
    vals.insert(
        "category_general".into(),
        bit_at(&search.categories, 0, true).to_string(),
    );
    vals.insert(
        "category_anime".into(),
        bit_at(&search.categories, 1, false).to_string(),
    );
    vals.insert(
        "category_people".into(),
        bit_at(&search.categories, 2, false).to_string(),
    );
    vals.insert(
        "purity_sfw".into(),
        bit_at(&search.purity, 0, true).to_string(),
    );
    vals.insert(
        "purity_sketchy".into(),
        bit_at(&search.purity, 1, false).to_string(),
    );
    vals.insert(
        "purity_nsfw".into(),
        if api_key_present {
            bit_at(&search.purity, 2, false).to_string()
        } else {
            "false".into()
        },
    );
    vals.insert("sorting".into(), search.sorting.clone());
    vals.insert("order".into(), search.order.clone());
    vals.insert("ratios".into(), search.ratios.clone());
    vals.insert("atleast".into(), search.atleast.clone());
    vals
}

pub(super) fn prefer_label(prefer: WallhavenPrefer) -> String {
    match prefer {
        WallhavenPrefer::CollectionsThenSearch => "collections_then_search".into(),
        WallhavenPrefer::SearchOnly => "search_only".into(),
        WallhavenPrefer::CollectionsOnly => "collections_only".into(),
    }
}

pub(super) fn parse_prefer(s: &str) -> Option<WallhavenPrefer> {
    match s.trim().to_ascii_lowercase().as_str() {
        "collections_then_search" => Some(WallhavenPrefer::CollectionsThenSearch),
        "search_only" => Some(WallhavenPrefer::SearchOnly),
        "collections_only" => Some(WallhavenPrefer::CollectionsOnly),
        _ => None,
    }
}

pub(super) fn format_collections(collections: &[WallhavenCollection]) -> String {
    collections
        .iter()
        .map(|collection| {
            let base = format!("{}/{}", collection.username, collection.id);
            collection
                .label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
                .map_or(base.clone(), |label| format!("{base}:{label}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn parse_collections(value: &str) -> Vec<WallhavenCollection> {
    value
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }
            let (identity, label) =
                entry
                    .split_once(':')
                    .map_or((entry, None), |(identity, label)| {
                        let label = label.trim();
                        (
                            identity.trim(),
                            (!label.is_empty()).then(|| label.to_string()),
                        )
                    });
            let (username, id) = identity
                .split_once('/')
                .map_or((identity.trim(), 0), |(username, id)| {
                    (username.trim(), id.trim().parse::<u32>().unwrap_or(0))
                });
            Some(WallhavenCollection {
                username: username.to_string(),
                id,
                label,
            })
        })
        .collect()
}

pub(super) fn apply_block_draft(
    config: &mut Config,
    draft: &HashMap<String, String>,
    api_key_present: bool,
) {
    if !config
        .sources
        .iter()
        .any(|source| source.source_type == "wallhaven")
    {
        config.sources.push(default_wallhaven_source());
    }
    let Some(source) = first_source_mut(config) else {
        return;
    };
    if let Some(v) = draft.get("enabled") {
        source.enabled = App::parse_bool_like(v).unwrap_or(source.enabled);
    }
    if let Some(v) = draft.get("prefer") {
        if let Some(prefer) = parse_prefer(v) {
            source.prefer = Some(prefer);
        }
    }
    apply_search_draft_to_source(source, draft, api_key_present);
}

pub(super) fn first_source(config: &Config) -> Option<&SourceEntry> {
    config
        .sources
        .iter()
        .find(|source| source.source_type == "wallhaven")
}

fn first_source_mut(config: &mut Config) -> Option<&mut SourceEntry> {
    config
        .sources
        .iter_mut()
        .find(|source| source.source_type == "wallhaven")
}

pub(super) fn first_search(config: &Config) -> WallhavenSearch {
    first_source(config)
        .map(source_wallhaven_search)
        .unwrap_or_default()
}

pub(super) fn apply_search_draft(
    search: &mut WallhavenSearch,
    draft: &HashMap<String, String>,
    api_key_present: bool,
) {
    if let Some(v) = draft.get("search_q") {
        search.q = v.clone();
    }
    let category_general = draft
        .get("category_general")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.categories, 0, true));
    let category_anime = draft
        .get("category_anime")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.categories, 1, false));
    let category_people = draft
        .get("category_people")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.categories, 2, false));
    search.categories = bits_from_bools(category_general, category_anime, category_people);

    let purity_sfw = draft
        .get("purity_sfw")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.purity, 0, true));
    let purity_sketchy = draft
        .get("purity_sketchy")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.purity, 1, false));
    let mut purity_nsfw = draft
        .get("purity_nsfw")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(bit_at(&search.purity, 2, false));
    if !api_key_present {
        purity_nsfw = false;
    }
    search.purity = bits_from_bools(purity_sfw, purity_sketchy, purity_nsfw);
    if let Some(v) = draft.get("sorting") {
        search.sorting = v.clone();
    }
    if let Some(v) = draft.get("order") {
        search.order = v.clone();
    }
    if let Some(v) = draft.get("ratios") {
        search.ratios = v.clone();
    }
    if let Some(v) = draft.get("atleast") {
        search.atleast = v.clone();
    }
}

fn apply_search_draft_to_source(
    source: &mut SourceEntry,
    draft: &HashMap<String, String>,
    api_key_present: bool,
) {
    let mut search = source_wallhaven_search(source);
    apply_search_draft(&mut search, draft, api_key_present);
    source.query = Some(search.q);
    source.categories = Some(search.categories);
    source.purity = Some(search.purity);
    source.sorting = Some(search.sorting);
    source.order = Some(search.order);
    source.ratios = Some(search.ratios);
    source.atleast = Some(search.atleast);
}

pub(super) fn set_category_bit(source: &mut SourceEntry, index: usize, value: &str) {
    let Some(enabled) = App::parse_bool_like(value) else {
        return;
    };
    let search = source_wallhaven_search(source);
    let mut bits = [
        bit_at(&search.categories, 0, true),
        bit_at(&search.categories, 1, false),
        bit_at(&search.categories, 2, false),
    ];
    if let Some(bit) = bits.get_mut(index) {
        *bit = enabled;
    }
    source.categories = Some(bits_from_bools(bits[0], bits[1], bits[2]));
}

pub(super) fn set_purity_bit(source: &mut SourceEntry, index: usize, value: &str) {
    let Some(enabled) = App::parse_bool_like(value) else {
        return;
    };
    let search = source_wallhaven_search(source);
    let mut bits = [
        bit_at(&search.purity, 0, true),
        bit_at(&search.purity, 1, false),
        bit_at(&search.purity, 2, false),
    ];
    if let Some(bit) = bits.get_mut(index) {
        *bit = enabled;
    }
    source.purity = Some(bits_from_bools(bits[0], bits[1], bits[2]));
}

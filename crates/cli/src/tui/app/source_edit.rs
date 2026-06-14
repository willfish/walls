use walls_core::config::{
    reddit_sort_needs_time, reddit_sort_value, reddit_time_value,
    source_editable_fields as core_source_editable_fields, source_field_preserves_blank,
    source_wallhaven_search, SourceEntry, REDDIT_SORT_CHOICES, REDDIT_TIME_CHOICES,
};

use super::wallhaven_edit;

pub(super) fn source_editable_fields(src: &SourceEntry) -> Vec<String> {
    core_source_editable_fields(src)
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(super) fn get_source_field(src: &SourceEntry, name: &str) -> String {
    match name {
        "enabled" => src.enabled.to_string(),
        "type" => src.source_type.clone(),
        "label" => src.label.clone().unwrap_or_default(),
        "url" => src.url.clone().unwrap_or_default(),
        "path" => src.path.clone().unwrap_or_default(),
        "image_path" => src.image_path.clone().unwrap_or_default(),
        "source" => src.source.clone().unwrap_or_default(),
        "author" => src.author.clone().unwrap_or_default(),
        "query" => src.query.clone().unwrap_or_default(),
        "required_tags" => src.required_tags.join(", "),
        "excluded_tags" => src.excluded_tags.join(", "),
        "api_key" => src.api_key.clone().unwrap_or_default(),
        "collection" => src.collection.clone().unwrap_or_default(),
        "user" => src.user.clone().unwrap_or_default(),
        "topic" => src.topic.clone().unwrap_or_default(),
        "orientation" => src.orientation.clone().unwrap_or_default(),
        "sort" => reddit_sort_value(src).to_string(),
        "category_general" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).categories, 0, true).to_string()
        }
        "category_anime" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).categories, 1, false).to_string()
        }
        "category_people" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).categories, 2, false).to_string()
        }
        "purity_sfw" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).purity, 0, true).to_string()
        }
        "purity_sketchy" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).purity, 1, false).to_string()
        }
        "purity_nsfw" => {
            wallhaven_edit::bit_at(&source_wallhaven_search(src).purity, 2, false).to_string()
        }
        "sorting" => src.sorting.clone().unwrap_or_default(),
        "order" => src.order.clone().unwrap_or_default(),
        "ratios" => src.ratios.clone().unwrap_or_default(),
        "atleast" => src.atleast.clone().unwrap_or_default(),
        "broaden_when_cache_below" => src
            .broaden_when_cache_below
            .map(|threshold| threshold.to_string())
            .unwrap_or_default(),
        "prefer" => src
            .prefer
            .map(wallhaven_edit::prefer_label)
            .unwrap_or_default(),
        "collections" => wallhaven_edit::format_collections(&src.collections),
        "time" => {
            if reddit_sort_needs_time(reddit_sort_value(src)) {
                reddit_time_value(src).to_string()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Lenient bool parser for edit buffers (user may type t/f/1/0/yes/no/on/off/true/false).
pub(super) fn parse_bool_like(s: &str) -> Option<bool> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "true" | "t" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "f" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

pub(super) fn set_source_field(draft: &mut SourceEntry, name: &str, buf: &str) {
    let trimmed = buf.trim();
    let v = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    match name {
        "enabled" => {
            draft.enabled = parse_bool_like(trimmed).unwrap_or(draft.enabled);
        }
        "type" => {
            draft.source_type = trimmed.to_string();
        }
        "label" => draft.label = v,
        "url" => draft.url = v,
        "path" => draft.path = v,
        "image_path" => draft.image_path = v,
        "source" => draft.source = v,
        "author" => draft.author = v,
        "query" if source_field_preserves_blank(&draft.source_type, name) => {
            draft.query = Some(trimmed.to_string());
        }
        "query" => draft.query = v,
        "required_tags" => draft.required_tags = parse_tag_list(buf),
        "excluded_tags" => draft.excluded_tags = parse_tag_list(buf),
        "api_key" => draft.api_key = v,
        "collection" => draft.collection = v,
        "user" => draft.user = v,
        "topic" => draft.topic = v,
        "orientation" => draft.orientation = v,
        "category_general" => wallhaven_edit::set_category_bit(draft, 0, trimmed),
        "category_anime" => wallhaven_edit::set_category_bit(draft, 1, trimmed),
        "category_people" => wallhaven_edit::set_category_bit(draft, 2, trimmed),
        "purity_sfw" => wallhaven_edit::set_purity_bit(draft, 0, trimmed),
        "purity_sketchy" => wallhaven_edit::set_purity_bit(draft, 1, trimmed),
        "purity_nsfw" => wallhaven_edit::set_purity_bit(draft, 2, trimmed),
        "sorting" => draft.sorting = v,
        "order" => draft.order = v,
        "ratios" => draft.ratios = v,
        "atleast" => draft.atleast = v,
        "broaden_when_cache_below" => {
            draft.broaden_when_cache_below = trimmed.parse::<usize>().ok();
        }
        "prefer" => {
            if let Some(prefer) = wallhaven_edit::parse_prefer(trimmed) {
                draft.prefer = Some(prefer);
            }
        }
        "collections" => {
            draft.collections = wallhaven_edit::parse_collections(buf);
        }
        "sort" if !trimmed.is_empty() && REDDIT_SORT_CHOICES.contains(&trimmed) => {
            draft.sort = Some(trimmed.to_string());
            if !reddit_sort_needs_time(trimmed) {
                draft.time = None;
            } else if draft
                .time
                .as_deref()
                .is_none_or(|t| !REDDIT_TIME_CHOICES.contains(&t))
            {
                draft.time = Some("week".into());
            }
        }
        "time"
            if !trimmed.is_empty()
                && REDDIT_TIME_CHOICES.contains(&trimmed)
                && reddit_sort_needs_time(reddit_sort_value(draft)) =>
        {
            draft.time = Some(trimmed.to_string());
        }
        _ => {}
    }
}

fn parse_tag_list(buf: &str) -> Vec<String> {
    let tags = buf
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    walls_core::config::normalize_wallhaven_tags(&tags)
}

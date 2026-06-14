use std::collections::HashMap;

use walls_core::config::{
    default_wallhaven_source, reddit_sort_needs_time, reddit_sort_value, Config, SourceEntry,
    WallhavenSearch, REDDIT_SORT_CHOICES, REDDIT_TIME_CHOICES,
};

use super::{config_block_edit, wallhaven_edit, App};

/// Internal block index for shared Wallhaven field metadata helpers.
pub(crate) const WALLHAVEN_FIELDS_BLOCK: usize = usize::MAX;
pub(crate) const CONFIG_BLOCK_SOURCES: usize = 0;
pub(crate) const CONFIG_BLOCK_ROTATION: usize = 1;
pub(crate) const CONFIG_BLOCK_LIBRARY: usize = 2;
pub(crate) const CONFIG_BLOCK_APPLY_DISPLAY: usize = 3;
pub(crate) const CONFIG_BLOCK_TUI: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditFieldKind {
    Text,
    TagList,
    Bool,
    Choice(&'static [&'static str]),
}

pub(crate) const SOURCE_TYPE_CHOICES: &[&str] = &[
    "folder",
    "image",
    "favorites",
    "fetched",
    "json",
    "mediarss",
    "attribution",
    "unsplash",
    "reddit",
    "weighting",
    "pixabay",
    "immich",
    "bing",
    "apod",
    "spotlight",
    "wallhaven",
];

pub(crate) const ROTATION_BLOCK_FIELDS: &[&str] = &[
    "enabled",
    "on_start",
    "interval",
    "internet",
    "safe_mode",
    "change_lock_screen",
    "download_preference_ratio",
    "tray_accent",
    "tray_autostart",
];

pub(crate) const WALLHAVEN_BLOCK_FIELDS: &[&str] = &[
    "enabled",
    "prefer",
    "search_q",
    "category_general",
    "category_anime",
    "category_people",
    "purity_sfw",
    "purity_sketchy",
    "purity_nsfw",
    "sorting",
    "order",
    "ratios",
    "atleast",
];

pub(crate) const SEARCH_FILTER_FIELDS: &[&str] = &[
    "search_q",
    "category_general",
    "category_anime",
    "category_people",
    "purity_sfw",
    "purity_sketchy",
    "purity_nsfw",
    "sorting",
    "order",
    "ratios",
    "atleast",
];

pub(crate) const TUI_BLOCK_FIELDS: &[&str] = &["key_profile"];
pub(crate) const TUI_KEY_PROFILE_CHOICES: &[&str] = &["emacs", "vim"];
pub(crate) const APPLY_BACKEND_CHOICES: &[&str] = &[
    "auto",
    "cosmic",
    "cosmic-ext-bg-ctl",
    "gnome",
    "kde",
    "xfce",
    "sway",
    "wlroots",
    "hyprland",
    "feh",
    "custom-script",
];
pub(crate) const COSMIC_METHOD_CHOICES: &[&str] = &["cosmic-config", "cosmic-ext-bg-ctl"];
pub(crate) const DISPLAY_MODE_CHOICES: &[&str] = &[
    "os",
    "zoom",
    "fill-with-black",
    "fill-with-blur",
    "spanned",
    "centered",
    "scaled",
    "stretched",
    "wallpaper",
];
pub(crate) const LIBRARY_BLOCK_FIELDS: &[&str] = &[
    "quota_enabled",
    "quota_size_mb",
    "use_landscape_enabled",
    "avoid_recent",
    "refetch_when_cache_below",
];
pub(crate) const APPLY_DISPLAY_BLOCK_FIELDS: &[&str] = &[
    "apply_backend",
    "custom_script",
    "cosmic_method",
    "cosmic_config_path",
    "cosmic_use_original_path",
    "display_mode",
    "auto_rotate",
    "imagemagick_command",
    "target_width",
    "target_height",
    "filters_enabled",
    "filters_command",
];

pub(crate) fn block_field_label(block: usize, key: &str) -> String {
    match block {
        CONFIG_BLOCK_ROTATION => match key {
            "enabled" => "Enabled".into(),
            "on_start" => "On start".into(),
            "interval" => "Interval (seconds)".into(),
            "internet" => "Internet enabled".into(),
            "safe_mode" => "Safe mode".into(),
            "change_lock_screen" => "Change lock screen".into(),
            "download_preference_ratio" => "Download preference ratio (0.0-1.0)".into(),
            "tray_accent" => "Tray icon accent".into(),
            "tray_autostart" => walls_core::autostart::tray_autostart_field_label(
                walls_core::autostart::current_autostart_desktop(),
            ),
            other => other.into(),
        },
        CONFIG_BLOCK_LIBRARY => match key {
            "quota_enabled" => "Quota enabled".into(),
            "quota_size_mb" => "Quota size (MB)".into(),
            "use_landscape_enabled" => "Use landscape filter".into(),
            "avoid_recent" => "Avoid recent count".into(),
            "refetch_when_cache_below" => "Refetch below cached count".into(),
            other => other.into(),
        },
        CONFIG_BLOCK_APPLY_DISPLAY => match key {
            "apply_backend" => "Apply backend".into(),
            "custom_script" => "Custom script".into(),
            "cosmic_method" => "COSMIC method".into(),
            "cosmic_config_path" => "COSMIC config path".into(),
            "cosmic_use_original_path" => "COSMIC use original".into(),
            "display_mode" => "Display mode".into(),
            "auto_rotate" => "EXIF auto-rotate".into(),
            "imagemagick_command" => "ImageMagick command".into(),
            "target_width" => "Target width".into(),
            "target_height" => "Target height".into(),
            "filters_enabled" => "Filters enabled".into(),
            "filters_command" => "Filter command".into(),
            other => other.into(),
        },
        WALLHAVEN_FIELDS_BLOCK => match key {
            "enabled" => "Enabled".into(),
            "prefer" => "Prefer".into(),
            "search_q" => "Search query".into(),
            "category_general" => "Category: General".into(),
            "category_anime" => "Category: Anime".into(),
            "category_people" => "Category: People".into(),
            "purity_sfw" => "Purity: SFW".into(),
            "purity_sketchy" => "Purity: Sketchy".into(),
            "purity_nsfw" => "Purity: NSFW".into(),
            "sorting" => "Sorting".into(),
            "order" => "Order".into(),
            "ratios" => "Aspect ratio".into(),
            "atleast" => "Minimum resolution".into(),
            other => other.into(),
        },
        CONFIG_BLOCK_TUI => match key {
            "key_profile" => "Key profile".into(),
            other => other.into(),
        },
        _ => key.into(),
    }
}

pub(crate) fn block_field_kind(block: usize, key: &str) -> EditFieldKind {
    match block {
        CONFIG_BLOCK_ROTATION => match key {
            "enabled" | "on_start" | "internet" | "safe_mode" | "change_lock_screen" => {
                EditFieldKind::Bool
            }
            "tray_autostart" => {
                if walls_core::autostart::tray_autostart_available(
                    walls_core::autostart::current_autostart_desktop(),
                ) {
                    EditFieldKind::Bool
                } else {
                    EditFieldKind::Text
                }
            }
            "tray_accent" => EditFieldKind::Choice(walls_core::tray_icon::tray_accent_choices()),
            _ => EditFieldKind::Text,
        },
        CONFIG_BLOCK_LIBRARY => match key {
            "quota_enabled" | "use_landscape_enabled" => EditFieldKind::Bool,
            _ => EditFieldKind::Text,
        },
        CONFIG_BLOCK_APPLY_DISPLAY => match key {
            "apply_backend" => EditFieldKind::Choice(APPLY_BACKEND_CHOICES),
            "cosmic_method" => EditFieldKind::Choice(COSMIC_METHOD_CHOICES),
            "cosmic_use_original_path" | "auto_rotate" | "filters_enabled" => EditFieldKind::Bool,
            "display_mode" => EditFieldKind::Choice(DISPLAY_MODE_CHOICES),
            _ => EditFieldKind::Text,
        },
        WALLHAVEN_FIELDS_BLOCK => match key {
            "enabled" => EditFieldKind::Bool,
            "prefer" => EditFieldKind::Choice(&[
                "collections_then_search",
                "search_only",
                "collections_only",
            ]),
            "category_general" | "category_anime" | "category_people" | "purity_sfw"
            | "purity_sketchy" | "purity_nsfw" => EditFieldKind::Bool,
            "sorting" => EditFieldKind::Choice(&[
                "date",
                "relevance",
                "random",
                "views",
                "favorites",
                "toplist",
            ]),
            "order" => EditFieldKind::Choice(&["desc", "asc"]),
            "ratios" => EditFieldKind::Choice(walls_core::config::wallhaven_ratio_choices()),
            "atleast" => EditFieldKind::Choice(walls_core::config::wallhaven_resolution_choices()),
            _ => EditFieldKind::Text,
        },
        CONFIG_BLOCK_TUI => match key {
            "key_profile" => EditFieldKind::Choice(TUI_KEY_PROFILE_CHOICES),
            _ => EditFieldKind::Text,
        },
        _ => EditFieldKind::Text,
    }
}

pub(crate) fn source_field_kind(name: &str) -> EditFieldKind {
    match name {
        "enabled" => EditFieldKind::Bool,
        "type" => EditFieldKind::Choice(SOURCE_TYPE_CHOICES),
        "orientation" => EditFieldKind::Choice(&["", "landscape", "portrait", "squarish"]),
        "sort" => EditFieldKind::Choice(REDDIT_SORT_CHOICES),
        "time" => EditFieldKind::Choice(REDDIT_TIME_CHOICES),
        _ => EditFieldKind::Text,
    }
}

pub(crate) fn source_field_kind_for(src: &SourceEntry, name: &str) -> EditFieldKind {
    if src.source_type == "reddit" {
        return source_field_kind(name);
    }
    if src.source_type == "wallhaven" {
        return match name {
            "enabled" => EditFieldKind::Bool,
            "type" => EditFieldKind::Choice(SOURCE_TYPE_CHOICES),
            "required_tags" | "excluded_tags" => EditFieldKind::TagList,
            "category_general" | "category_anime" | "category_people" | "purity_sfw"
            | "purity_sketchy" | "purity_nsfw" => EditFieldKind::Bool,
            "prefer" => EditFieldKind::Choice(&[
                "collections_then_search",
                "search_only",
                "collections_only",
            ]),
            "sorting" => EditFieldKind::Choice(&[
                "date",
                "relevance",
                "random",
                "views",
                "favorites",
                "toplist",
            ]),
            "order" => EditFieldKind::Choice(&["desc", "asc"]),
            "ratios" => EditFieldKind::Choice(walls_core::config::wallhaven_ratio_choices()),
            "atleast" => EditFieldKind::Choice(walls_core::config::wallhaven_resolution_choices()),
            _ => EditFieldKind::Text,
        };
    }
    match name {
        "sort" | "time" => EditFieldKind::Text,
        _ => source_field_kind(name),
    }
}

pub(crate) fn reddit_time_field_locked(src: &SourceEntry) -> bool {
    src.source_type == "reddit" && !reddit_sort_needs_time(reddit_sort_value(src))
}

pub(crate) fn source_field_label(src: &SourceEntry, name: &str) -> String {
    if src.source_type == "reddit" {
        return match name {
            "enabled" => "Enabled".into(),
            "query" => "Subreddit".into(),
            "sort" => "Sort".into(),
            "time" => "Time period".into(),
            other => other.into(),
        };
    }
    if src.source_type == "wallhaven" {
        return match name {
            "enabled" => "Enabled".into(),
            "type" => "Type".into(),
            "query" => "Search query".into(),
            "required_tags" => "Required tags".into(),
            "excluded_tags" => "Excluded tags".into(),
            "category_general" => "Category: General".into(),
            "category_anime" => "Category: Anime".into(),
            "category_people" => "Category: People".into(),
            "purity_sfw" => "Purity: SFW".into(),
            "purity_sketchy" => "Purity: Sketchy".into(),
            "purity_nsfw" => "Purity: NSFW".into(),
            "sorting" => "Sorting".into(),
            "order" => "Order".into(),
            "ratios" => "Aspect ratio".into(),
            "atleast" => "Minimum resolution".into(),
            "broaden_when_cache_below" => "Broaden below".into(),
            "prefer" => "Prefer".into(),
            "collections" => "Collections".into(),
            other => other.into(),
        };
    }
    match name {
        "enabled" => "Enabled".into(),
        "type" => "Type".into(),
        "label" => "Label".into(),
        "url" => "URL".into(),
        "path" => "Path".into(),
        "image_path" => "Image path (JSONPath)".into(),
        "source" => "Source".into(),
        "author" => "Author".into(),
        "query" => "Query".into(),
        "api_key" => "API key".into(),
        "collection" => "Collection".into(),
        "user" => "User".into(),
        "topic" => "Topic".into(),
        "orientation" => "Orientation".into(),
        other => other.into(),
    }
}

pub(super) fn cycle_choice_value(current: &str, options: &[&str], forward: bool) -> String {
    if options.is_empty() {
        return current.to_string();
    }
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    let next = if forward {
        (idx + 1) % options.len()
    } else {
        (idx + options.len().saturating_sub(1)) % options.len()
    };
    options[next].to_string()
}

pub(super) fn toggle_bool_value(current: &str) -> String {
    if App::parse_bool_like(current) == Some(true) {
        "false".into()
    } else {
        "true".into()
    }
}

pub(super) fn choice_display_value(kind: EditFieldKind, value: &str) -> String {
    match kind {
        EditFieldKind::Bool => {
            if App::parse_bool_like(value) == Some(true) {
                "true".into()
            } else {
                "false".into()
            }
        }
        EditFieldKind::Choice(options) => {
            if value.is_empty() && options.first() == Some(&"") {
                "(any)".into()
            } else {
                value.to_string()
            }
        }
        EditFieldKind::TagList => value.to_string(),
        EditFieldKind::Text => value.to_string(),
    }
}

pub(super) fn block_field_value_at(
    config: &Config,
    block: usize,
    draft: &HashMap<String, String>,
    idx: usize,
) -> String {
    let keys = match block {
        CONFIG_BLOCK_ROTATION => ROTATION_BLOCK_FIELDS,
        CONFIG_BLOCK_LIBRARY => LIBRARY_BLOCK_FIELDS,
        CONFIG_BLOCK_APPLY_DISPLAY => APPLY_DISPLAY_BLOCK_FIELDS,
        CONFIG_BLOCK_TUI => TUI_BLOCK_FIELDS,
        WALLHAVEN_FIELDS_BLOCK => WALLHAVEN_BLOCK_FIELDS,
        _ => return String::new(),
    };
    let Some(key) = keys.get(idx) else {
        return String::new();
    };
    if let Some(v) = draft.get(*key) {
        return v.clone();
    }
    match block {
        CONFIG_BLOCK_ROTATION => match *key {
            "enabled" => config.change.enabled.to_string(),
            "on_start" => config.change.on_start.to_string(),
            "interval" => config.change.interval_secs.to_string(),
            "internet" => config.change.internet_enabled.to_string(),
            "safe_mode" => config.change.safe_mode.to_string(),
            "change_lock_screen" => config.change.change_lock_screen.to_string(),
            "download_preference_ratio" => config.change.download_preference_ratio.to_string(),
            "tray_accent" => walls_core::tray_icon::tray_accent_label(
                walls_core::tray_icon::effective_tray_accent(config.tray.accent),
            )
            .into(),
            "tray_autostart" => {
                let desktop = walls_core::autostart::current_autostart_desktop();
                if walls_core::autostart::tray_autostart_available(desktop) {
                    walls_core::autostart::tray_autostart_enabled_for_desktop(config, desktop)
                        .to_string()
                } else {
                    "unavailable".into()
                }
            }
            _ => String::new(),
        },
        CONFIG_BLOCK_LIBRARY => match *key {
            "quota_enabled" => config.quota.enabled.to_string(),
            "quota_size_mb" => config.quota.size_mb.to_string(),
            "use_landscape_enabled" => config.selection.use_landscape_enabled.to_string(),
            "avoid_recent" => config.selection.avoid_recent.to_string(),
            "refetch_when_cache_below" => config.selection.refetch_when_cache_below.to_string(),
            _ => String::new(),
        },
        CONFIG_BLOCK_APPLY_DISPLAY => match *key {
            "apply_backend" => config_block_edit::apply_backend_label(config.apply.backend).into(),
            "custom_script" => config.apply.custom_script.clone().unwrap_or_default(),
            "cosmic_method" => {
                config_block_edit::cosmic_method_label(config.apply.cosmic.method).into()
            }
            "cosmic_config_path" => config.apply.cosmic.config_path.clone(),
            "cosmic_use_original_path" => config.apply.cosmic.use_original_path.to_string(),
            "display_mode" => config.display.mode.clone(),
            "auto_rotate" => config.display.auto_rotate.to_string(),
            "imagemagick_command" => config.display.imagemagick_command.clone(),
            "target_width" => config
                .display
                .target_width
                .map(|width| width.to_string())
                .unwrap_or_default(),
            "target_height" => config
                .display
                .target_height
                .map(|height| height.to_string())
                .unwrap_or_default(),
            "filters_enabled" => config.display.filters.enabled.to_string(),
            "filters_command" => config.display.filters.command.clone(),
            _ => String::new(),
        },
        WALLHAVEN_FIELDS_BLOCK => match *key {
            "enabled" => wallhaven_edit::first_source(config)
                .is_some_and(|source| source.enabled)
                .to_string(),
            "prefer" => wallhaven_edit::prefer_label(
                wallhaven_edit::first_source(config)
                    .map(walls_core::config::source_wallhaven_prefer)
                    .unwrap_or_default(),
            ),
            "search_q" => wallhaven_edit::first_search(config).q,
            "category_general" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).categories, 0, true)
                    .to_string()
            }
            "category_anime" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).categories, 1, false)
                    .to_string()
            }
            "category_people" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).categories, 2, false)
                    .to_string()
            }
            "purity_sfw" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).purity, 0, true)
                    .to_string()
            }
            "purity_sketchy" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).purity, 1, false)
                    .to_string()
            }
            "purity_nsfw" => {
                wallhaven_edit::bit_at(&wallhaven_edit::first_search(config).purity, 2, false)
                    .to_string()
            }
            "sorting" => wallhaven_edit::first_search(config).sorting,
            "order" => wallhaven_edit::first_search(config).order,
            "ratios" => wallhaven_edit::first_search(config).ratios,
            "atleast" => wallhaven_edit::first_search(config).atleast,
            _ => String::new(),
        },
        CONFIG_BLOCK_TUI => match *key {
            "key_profile" => {
                config_block_edit::tui_key_profile_label(config.tui.key_profile).into()
            }
            _ => String::new(),
        },
        _ => String::new(),
    }
}

pub(super) fn search_filter_field_value_at(
    search: &WallhavenSearch,
    draft: &HashMap<String, String>,
    idx: usize,
) -> String {
    let Some(key) = SEARCH_FILTER_FIELDS.get(idx) else {
        return String::new();
    };
    if let Some(v) = draft.get(*key) {
        return v.clone();
    }
    match *key {
        "search_q" => search.q.clone(),
        "category_general" => wallhaven_edit::bit_at(&search.categories, 0, true).to_string(),
        "category_anime" => wallhaven_edit::bit_at(&search.categories, 1, false).to_string(),
        "category_people" => wallhaven_edit::bit_at(&search.categories, 2, false).to_string(),
        "purity_sfw" => wallhaven_edit::bit_at(&search.purity, 0, true).to_string(),
        "purity_sketchy" => wallhaven_edit::bit_at(&search.purity, 1, false).to_string(),
        "purity_nsfw" => wallhaven_edit::bit_at(&search.purity, 2, false).to_string(),
        "sorting" => search.sorting.clone(),
        "order" => search.order.clone(),
        "ratios" => search.ratios.clone(),
        "atleast" => search.atleast.clone(),
        _ => String::new(),
    }
}

pub(super) fn commit_block_field_buffer(
    block: usize,
    field_idx: usize,
    buf: &str,
    draft: &mut HashMap<String, String>,
) {
    let keys = match block {
        CONFIG_BLOCK_ROTATION => ROTATION_BLOCK_FIELDS,
        CONFIG_BLOCK_LIBRARY => LIBRARY_BLOCK_FIELDS,
        CONFIG_BLOCK_APPLY_DISPLAY => APPLY_DISPLAY_BLOCK_FIELDS,
        CONFIG_BLOCK_TUI => TUI_BLOCK_FIELDS,
        WALLHAVEN_FIELDS_BLOCK => WALLHAVEN_BLOCK_FIELDS,
        _ => return,
    };
    let Some(key) = keys.get(field_idx) else {
        return;
    };
    draft.insert((*key).into(), buf.trim().to_string());
}

pub(super) fn default_wallhaven_source_entry() -> SourceEntry {
    default_wallhaven_source()
}

pub(super) fn source_entry_display_name(source: &SourceEntry) -> String {
    if source.source_type == "wallhaven" {
        if let Some(query) = source
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
        {
            return format!("Wallhaven {query}");
        }
        if !source.required_tags.is_empty() {
            return format!("Wallhaven tags: {}", source.required_tags.join(", "));
        }
        return "Wallhaven".into();
    }
    source
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or(&source.source_type)
        .to_string()
}

pub(super) fn source_removal_protected(source: &SourceEntry) -> bool {
    matches!(source.source_type.as_str(), "favorites" | "fetched")
}

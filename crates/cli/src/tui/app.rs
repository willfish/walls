use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::config::{
    normalize_source_entry, persist_config, reddit_sort_needs_time, reddit_sort_value,
    reddit_time_value, source_editable_fields as core_source_editable_fields, Config,
    SelectionStrategy, SourceEntry, TuiKeyProfile, WallhavenPrefer, REDDIT_SORT_CHOICES,
    REDDIT_TIME_CHOICES,
};
use walls_core::expand_home;
use walls_core::sources::list_images_with_paths;
use walls_core::validate::{
    validate_config_diagnostics, validate_source_edit, validate_wallhaven_edit,
};
use walls_core::WallsCtx;

use super::command::{self, ParsedCommand};
use super::history_browse_view;
use super::style::{self, ColorMode, StateKind, StatusKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Config,
    Now,
    History,
    Browse,
    Search,
    Logs,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Config => 0,
            Tab::Now => 1,
            Tab::History => 2,
            Tab::Browse => 3,
            Tab::Search => 4,
            Tab::Logs => 5,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Config => "Config",
            Tab::Now => "Now",
            Tab::History => "History",
            Tab::Browse => "Browse",
            Tab::Search => "Search",
            Tab::Logs => "Logs",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Config,
            1 => Tab::Now,
            2 => Tab::History,
            3 => Tab::Browse,
            4 => Tab::Search,
            5 => Tab::Logs,
            _ => Tab::Config,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
    SearchInput,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditTarget {
    Block(usize),
    Source(usize),
    Wallhaven,
}

/// Internal block index for shared Wallhaven field metadata helpers.
pub(crate) const WALLHAVEN_FIELDS_BLOCK: usize = usize::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditFieldKind {
    Text,
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
    "atleast",
];

pub(crate) const TUI_BLOCK_FIELDS: &[&str] = &["key_profile"];
pub(crate) const TUI_KEY_PROFILE_CHOICES: &[&str] = &["emacs", "vim"];
pub(crate) const LIBRARY_BLOCK_FIELDS: &[&str] = &["quota_enabled", "quota_size_mb"];

fn wallhaven_bit_at(s: &str, idx: usize, default: bool) -> bool {
    s.chars().nth(idx).map(|c| c == '1').unwrap_or(default)
}

fn wallhaven_bits_from_bools(a: bool, b: bool, c: bool) -> String {
    format!("{}{}{}", u8::from(a), u8::from(b), u8::from(c))
}

pub(crate) fn format_wallhaven_categories(s: &str) -> String {
    let mut parts = Vec::new();
    if wallhaven_bit_at(s, 0, true) {
        parts.push("general");
    }
    if wallhaven_bit_at(s, 1, false) {
        parts.push("anime");
    }
    if wallhaven_bit_at(s, 2, false) {
        parts.push("people");
    }
    if parts.is_empty() {
        "(none)".into()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn format_wallhaven_purity(s: &str, api_key_present: bool) -> String {
    let mut parts = Vec::new();
    if wallhaven_bit_at(s, 0, true) {
        parts.push("SFW");
    }
    if wallhaven_bit_at(s, 1, false) {
        parts.push("sketchy");
    }
    if api_key_present && wallhaven_bit_at(s, 2, false) {
        parts.push("NSFW");
    }
    if parts.is_empty() {
        "(none)".into()
    } else {
        parts.join(", ")
    }
}

fn rotation_block_draft(config: &Config) -> std::collections::HashMap<String, String> {
    let mut vals = std::collections::HashMap::new();
    vals.insert("enabled".into(), config.change.enabled.to_string());
    vals.insert("on_start".into(), config.change.on_start.to_string());
    vals.insert("interval".into(), config.change.interval_secs.to_string());
    vals.insert(
        "internet".into(),
        config.change.internet_enabled.to_string(),
    );
    vals.insert("safe_mode".into(), config.change.safe_mode.to_string());
    vals.insert(
        "change_lock_screen".into(),
        config.change.change_lock_screen.to_string(),
    );
    vals.insert(
        "download_preference_ratio".into(),
        config.change.download_preference_ratio.to_string(),
    );
    vals.insert(
        "tray_accent".into(),
        walls_core::tray_icon::tray_accent_label(walls_core::tray_icon::effective_tray_accent(
            config.tray.accent,
        ))
        .into(),
    );
    let desktop = walls_core::autostart::current_autostart_desktop();
    vals.insert(
        "tray_autostart".into(),
        walls_core::autostart::tray_autostart_enabled_for_desktop(config, desktop).to_string(),
    );
    vals
}

fn wallhaven_api_key_present(secrets: &walls_core::config::Secrets) -> bool {
    !secrets.wallhaven_api_key.trim().is_empty()
}

fn wallhaven_block_draft(
    config: &Config,
    api_key_present: bool,
) -> std::collections::HashMap<String, String> {
    let search = &config.wallhaven.search;
    let mut vals = std::collections::HashMap::new();
    vals.insert("enabled".into(), config.wallhaven.enabled.to_string());
    vals.insert(
        "prefer".into(),
        wallhaven_prefer_label(config.wallhaven.prefer),
    );
    vals.insert("search_q".into(), search.q.clone());
    vals.insert(
        "category_general".into(),
        wallhaven_bit_at(&search.categories, 0, true).to_string(),
    );
    vals.insert(
        "category_anime".into(),
        wallhaven_bit_at(&search.categories, 1, false).to_string(),
    );
    vals.insert(
        "category_people".into(),
        wallhaven_bit_at(&search.categories, 2, false).to_string(),
    );
    vals.insert(
        "purity_sfw".into(),
        wallhaven_bit_at(&search.purity, 0, true).to_string(),
    );
    vals.insert(
        "purity_sketchy".into(),
        wallhaven_bit_at(&search.purity, 1, false).to_string(),
    );
    vals.insert(
        "purity_nsfw".into(),
        if api_key_present {
            wallhaven_bit_at(&search.purity, 2, false).to_string()
        } else {
            "false".into()
        },
    );
    vals.insert("sorting".into(), search.sorting.clone());
    vals.insert("order".into(), search.order.clone());
    vals.insert("atleast".into(), search.atleast.clone());
    vals
}

fn wallhaven_prefer_label(prefer: WallhavenPrefer) -> String {
    match prefer {
        WallhavenPrefer::CollectionsThenSearch => "collections_then_search".into(),
        WallhavenPrefer::SearchOnly => "search_only".into(),
        WallhavenPrefer::CollectionsOnly => "collections_only".into(),
    }
}

fn parse_wallhaven_prefer(s: &str) -> Option<WallhavenPrefer> {
    match s.trim().to_ascii_lowercase().as_str() {
        "collections_then_search" => Some(WallhavenPrefer::CollectionsThenSearch),
        "search_only" => Some(WallhavenPrefer::SearchOnly),
        "collections_only" => Some(WallhavenPrefer::CollectionsOnly),
        _ => None,
    }
}

pub(crate) fn block_field_label(block: usize, key: &str) -> String {
    match block {
        0 => match key {
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
        2 => match key {
            "quota_enabled" => "Quota enabled".into(),
            "quota_size_mb" => "Quota size (MB)".into(),
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
            "purity_nsfw" => "Purity: NSFW (requires API key)".into(),
            "sorting" => "Sorting".into(),
            "order" => "Order".into(),
            "atleast" => "Minimum resolution".into(),
            other => other.into(),
        },
        4 => match key {
            "key_profile" => "Key profile".into(),
            other => other.into(),
        },
        _ => key.into(),
    }
}

pub(crate) fn block_field_kind(block: usize, key: &str) -> EditFieldKind {
    match block {
        0 => match key {
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
        2 => match key {
            "quota_enabled" => EditFieldKind::Bool,
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
            "atleast" => EditFieldKind::Choice(walls_core::config::wallhaven_resolution_choices()),
            _ => EditFieldKind::Text,
        },
        4 => match key {
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
    match name {
        "enabled" => "Enabled".into(),
        "type" => "Type".into(),
        "label" => "Label".into(),
        "url" => "URL".into(),
        "path" => "Path".into(),
        "image_path" => "Image path (JSONPath)".into(),
        "query" => "Query".into(),
        "api_key" => "API key".into(),
        "collection" => "Collection".into(),
        "user" => "User".into(),
        "topic" => "Topic".into(),
        "orientation" => "Orientation".into(),
        other => other.into(),
    }
}

fn cycle_choice_value(current: &str, options: &[&str], forward: bool) -> String {
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

fn toggle_bool_value(current: &str) -> String {
    if App::parse_bool_like(current) == Some(true) {
        "false".into()
    } else {
        "true".into()
    }
}

fn choice_display_value(kind: EditFieldKind, value: &str) -> String {
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
        EditFieldKind::Text => value.to_string(),
    }
}

fn block_field_value_at(
    config: &Config,
    block: usize,
    draft: &std::collections::HashMap<String, String>,
    idx: usize,
) -> String {
    let keys = match block {
        0 => ROTATION_BLOCK_FIELDS,
        2 => LIBRARY_BLOCK_FIELDS,
        4 => TUI_BLOCK_FIELDS,
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
        0 => match *key {
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
        2 => match *key {
            "quota_enabled" => config.quota.enabled.to_string(),
            "quota_size_mb" => config.quota.size_mb.to_string(),
            _ => String::new(),
        },
        WALLHAVEN_FIELDS_BLOCK => match *key {
            "enabled" => config.wallhaven.enabled.to_string(),
            "prefer" => wallhaven_prefer_label(config.wallhaven.prefer),
            "search_q" => config.wallhaven.search.q.clone(),
            "category_general" => {
                wallhaven_bit_at(&config.wallhaven.search.categories, 0, true).to_string()
            }
            "category_anime" => {
                wallhaven_bit_at(&config.wallhaven.search.categories, 1, false).to_string()
            }
            "category_people" => {
                wallhaven_bit_at(&config.wallhaven.search.categories, 2, false).to_string()
            }
            "purity_sfw" => wallhaven_bit_at(&config.wallhaven.search.purity, 0, true).to_string(),
            "purity_sketchy" => {
                wallhaven_bit_at(&config.wallhaven.search.purity, 1, false).to_string()
            }
            "purity_nsfw" => {
                wallhaven_bit_at(&config.wallhaven.search.purity, 2, false).to_string()
            }
            "sorting" => config.wallhaven.search.sorting.clone(),
            "order" => config.wallhaven.search.order.clone(),
            "atleast" => config.wallhaven.search.atleast.clone(),
            _ => String::new(),
        },
        4 => match *key {
            "key_profile" => tui_key_profile_label(config.tui.key_profile).into(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn commit_block_field_buffer(
    block: usize,
    field_idx: usize,
    buf: &str,
    draft: &mut std::collections::HashMap<String, String>,
) {
    let keys = match block {
        0 => ROTATION_BLOCK_FIELDS,
        2 => LIBRARY_BLOCK_FIELDS,
        4 => TUI_BLOCK_FIELDS,
        WALLHAVEN_FIELDS_BLOCK => WALLHAVEN_BLOCK_FIELDS,
        _ => return,
    };
    let Some(key) = keys.get(field_idx) else {
        return;
    };
    draft.insert((*key).into(), buf.trim().to_string());
}

fn tui_block_draft(config: &Config) -> std::collections::HashMap<String, String> {
    let mut vals = std::collections::HashMap::new();
    vals.insert(
        "key_profile".into(),
        tui_key_profile_label(config.tui.key_profile).into(),
    );
    vals
}

fn library_block_draft(config: &Config) -> std::collections::HashMap<String, String> {
    let mut vals = std::collections::HashMap::new();
    vals.insert("quota_enabled".into(), config.quota.enabled.to_string());
    vals.insert("quota_size_mb".into(), config.quota.size_mb.to_string());
    vals
}

fn tui_key_profile_label(profile: TuiKeyProfile) -> &'static str {
    match profile {
        TuiKeyProfile::Emacs => "emacs",
        TuiKeyProfile::Vim => "vim",
    }
}

fn parse_tui_key_profile(value: &str) -> Option<TuiKeyProfile> {
    match value.trim().to_ascii_lowercase().as_str() {
        "emacs" | "default" => Some(TuiKeyProfile::Emacs),
        "vim" => Some(TuiKeyProfile::Vim),
        _ => None,
    }
}

fn apply_tui_block_draft(config: &mut Config, draft: &std::collections::HashMap<String, String>) {
    if let Some(profile) = draft
        .get("key_profile")
        .and_then(|v| parse_tui_key_profile(v))
    {
        config.tui.key_profile = profile;
    }
}

fn apply_library_block_draft(
    config: &mut Config,
    draft: &std::collections::HashMap<String, String>,
) {
    if let Some(v) = draft.get("quota_enabled") {
        config.quota.enabled = App::parse_bool_like(v).unwrap_or(config.quota.enabled);
    }
    if let Some(v) = draft.get("quota_size_mb") {
        if let Ok(size_mb) = v.parse::<u64>() {
            config.quota.size_mb = size_mb;
        }
    }
}

fn apply_rotation_block_draft(
    config: &mut Config,
    draft: &std::collections::HashMap<String, String>,
) {
    if let Some(v) = draft.get("enabled") {
        config.change.enabled = App::parse_bool_like(v).unwrap_or(config.change.enabled);
    }
    if let Some(v) = draft.get("on_start") {
        config.change.on_start = App::parse_bool_like(v).unwrap_or(config.change.on_start);
    }
    if let Some(v) = draft.get("interval") {
        if let Ok(n) = v.parse() {
            config.change.interval_secs = n;
        }
    }
    if let Some(v) = draft.get("internet") {
        config.change.internet_enabled =
            App::parse_bool_like(v).unwrap_or(config.change.internet_enabled);
    }
    if let Some(v) = draft.get("safe_mode") {
        config.change.safe_mode = App::parse_bool_like(v).unwrap_or(config.change.safe_mode);
    }
    if let Some(v) = draft.get("change_lock_screen") {
        config.change.change_lock_screen =
            App::parse_bool_like(v).unwrap_or(config.change.change_lock_screen);
    }
    if let Some(v) = draft.get("download_preference_ratio") {
        if let Ok(f) = v.parse::<f64>() {
            config.change.download_preference_ratio = f.clamp(0.0, 1.0);
        }
    }
    if let Some(v) = draft.get("tray_accent") {
        if let Some(accent) = walls_core::tray_icon::parse_tray_accent(v) {
            if walls_core::tray_icon::tray_accent_available(accent) {
                config.tray.accent = accent;
            }
        }
    }
    if let Some(v) = draft.get("tray_autostart") {
        let desktop = walls_core::autostart::current_autostart_desktop();
        if walls_core::autostart::tray_autostart_available(desktop) {
            if let Some(enabled) = App::parse_bool_like(v) {
                walls_core::autostart::set_tray_autostart_for_desktop(config, desktop, enabled);
            }
        }
    }
}

fn apply_wallhaven_block_draft(
    config: &mut Config,
    draft: &std::collections::HashMap<String, String>,
    api_key_present: bool,
) {
    if let Some(v) = draft.get("enabled") {
        config.wallhaven.enabled = App::parse_bool_like(v).unwrap_or(config.wallhaven.enabled);
    }
    if let Some(v) = draft.get("prefer") {
        if let Some(prefer) = parse_wallhaven_prefer(v) {
            config.wallhaven.prefer = prefer;
        }
    }
    if let Some(v) = draft.get("search_q") {
        config.wallhaven.search.q = v.clone();
    }
    let category_general = draft
        .get("category_general")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(
            &config.wallhaven.search.categories,
            0,
            true,
        ));
    let category_anime = draft
        .get("category_anime")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(
            &config.wallhaven.search.categories,
            1,
            false,
        ));
    let category_people = draft
        .get("category_people")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(
            &config.wallhaven.search.categories,
            2,
            false,
        ));
    config.wallhaven.search.categories =
        wallhaven_bits_from_bools(category_general, category_anime, category_people);

    let purity_sfw = draft
        .get("purity_sfw")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(&config.wallhaven.search.purity, 0, true));
    let purity_sketchy = draft
        .get("purity_sketchy")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(&config.wallhaven.search.purity, 1, false));
    let mut purity_nsfw = draft
        .get("purity_nsfw")
        .and_then(|v| App::parse_bool_like(v))
        .unwrap_or(wallhaven_bit_at(&config.wallhaven.search.purity, 2, false));
    if !api_key_present {
        purity_nsfw = false;
    }
    config.wallhaven.search.purity =
        wallhaven_bits_from_bools(purity_sfw, purity_sketchy, purity_nsfw);
    if let Some(v) = draft.get("sorting") {
        config.wallhaven.search.sorting = v.clone();
    }
    if let Some(v) = draft.get("order") {
        config.wallhaven.search.order = v.clone();
    }
    if let Some(v) = draft.get("atleast") {
        config.wallhaven.search.atleast = v.clone();
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EditSession {
    pub target: EditTarget,
    pub draft_source: Option<SourceEntry>,
    pub draft_block_values: std::collections::HashMap<String, String>,
    pub field_cursor: usize,
    pub field_buffer: String,
    pub validation_errors: Vec<String>,
}

pub struct SearchHit {
    pub id: String,
    pub label: String,
}

pub struct LocalSourceSummary {
    pub enabled: bool,
    pub source_type: String,
    pub label: String,
    pub path: String,
    pub status: String,
    pub candidates: usize,
}

pub struct WallhavenProviderSummary {
    pub enabled: bool,
    pub internet_enabled: bool,
    pub api_key_present: bool,
    pub prefer: String,
    pub collections: Vec<String>,
    pub query: String,
    pub categories: String,
    pub purity: String,
    pub sorting: String,
    pub order: String,
    pub atleast: String,
    pub warnings: Vec<String>,
}

pub struct App {
    pub ctx: WallsCtx,
    pub tab: Tab,
    pub config_cursor: usize,
    pub cursor: usize,
    pub logs_cursor: usize,
    logs_seen_len: usize,
    pub config_in_subnav: bool,
    pub config_sub_cursor: usize,
    pub message: String,
    pub message_kind: StatusKind,
    pub input_mode: InputMode,
    #[allow(dead_code)]
    pub editing: Option<EditSession>,
    pub cmd_line: String,
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
    pub(crate) local_source_summaries: Vec<LocalSourceSummary>,
    pub(crate) wallhaven_summary: WallhavenProviderSummary,
    pub(crate) config_warnings: Vec<String>,
    pub color_mode: ColorMode,
    pub pending_nuke_confirm: bool,
    pub pending_trash_confirm: bool,
    pub show_key_help: bool,
    pub vim_pending_g: bool,
}

impl App {
    const NORMAL_TAB_NAV_HINT: &'static str = "1-6/←/→ tabs";

    pub fn new(ctx: WallsCtx) -> anyhow::Result<Self> {
        let search_query = ctx.config.wallhaven.search.q.clone();
        let wallhaven_summary = summarize_wallhaven_provider(&ctx);
        let config_warnings = summarize_config_warnings(&ctx);
        let mut app = Self {
            ctx,
            tab: Tab::Config,
            config_cursor: 0,
            cursor: 0,
            logs_cursor: 0,
            logs_seen_len: super::log_len(),
            config_in_subnav: false,
            config_sub_cursor: 0,
            message: String::new(),
            message_kind: StatusKind::Neutral,
            input_mode: InputMode::Normal,
            editing: None,
            cmd_line: String::new(),
            search_query,
            search_results: Vec::new(),
            local_candidates: Vec::new(),
            local_source_summaries: Vec::new(),
            wallhaven_summary,
            config_warnings,
            color_mode: ColorMode::from_env(),
            pending_nuke_confirm: false,
            pending_trash_confirm: false,
            show_key_help: false,
            vim_pending_g: false,
        };
        app.refresh_local_candidates()?;
        Ok(app)
    }

    pub fn set_message(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.message = message.into();
        self.message_kind = kind;
    }

    pub fn clear_message(&mut self) {
        self.message.clear();
        self.message_kind = StatusKind::Neutral;
    }

    /// Reload config/state from disk. Recreates `config.json` with defaults if it was removed.
    pub fn reload_ctx(&mut self) -> anyhow::Result<()> {
        let paths = self.ctx.paths.clone();
        self.ctx = WallsCtx::load_with_paths(paths)?;
        self.refresh_local_candidates()?;
        Ok(())
    }

    fn refresh_local_candidates(&mut self) -> anyhow::Result<()> {
        self.local_candidates = self.ctx.collect_local_candidates().unwrap_or_default();
        self.local_source_summaries = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| is_local_source(source))
            .map(|source| summarize_local_source(&self.ctx, source))
            .collect();
        self.wallhaven_summary = summarize_wallhaven_provider(&self.ctx);
        self.config_warnings = summarize_config_warnings(&self.ctx);
        Ok(())
    }

    pub fn sources_subnav_len(&self) -> usize {
        self.ctx.config.sources.len() + 1
    }

    pub fn is_wallhaven_subnav_index(&self, idx: usize) -> bool {
        idx == self.ctx.config.sources.len()
    }

    pub fn move_down(&mut self) {
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            let len = self.sources_subnav_len();
            if len > 0 {
                self.config_sub_cursor = (self.config_sub_cursor + 1).min(len - 1);
            }
            return;
        }
        self.sync_log_cursor();
        let len = self.list_len();
        if len > 0 {
            let is_config = self.tab == Tab::Config;
            let old_block = if is_config {
                Some(self.config_cursor)
            } else {
                None
            };
            let cursor = self.active_cursor_mut();
            *cursor = (*cursor + 1).min(len - 1);
            if let Some(old_b) = old_block {
                let new_b = *cursor;
                if new_b != old_b && !self.is_sources_list_block(new_b) {
                    self.config_in_subnav = false;
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        self.sync_log_cursor();
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor = self.config_sub_cursor.saturating_sub(1);
            return;
        }
        let is_config = self.tab == Tab::Config;
        let old_block = if is_config {
            Some(self.config_cursor)
        } else {
            None
        };
        let cursor = self.active_cursor_mut();
        *cursor = (*cursor).saturating_sub(1);
        if let Some(old_b) = old_block {
            let new_b = *cursor;
            if new_b != old_b && !self.is_sources_list_block(new_b) {
                self.config_in_subnav = false;
            }
        }
    }

    pub fn move_first(&mut self) {
        self.move_to_row(0);
    }

    pub fn move_last(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.move_to_row(len - 1);
        }
    }

    pub fn page_up(&mut self) {
        let cursor = self.active_cursor_value();
        self.move_to_row(cursor.saturating_sub(5));
    }

    pub fn page_down(&mut self) {
        let len = self.list_len();
        if len > 0 {
            let cursor = self.active_cursor_value();
            self.move_to_row((cursor + 5).min(len - 1));
        }
    }

    fn active_cursor_value(&self) -> usize {
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor
        } else if self.tab == Tab::Config {
            self.config_cursor
        } else if self.tab == Tab::Logs {
            self.logs_cursor
        } else {
            self.cursor
        }
    }

    fn move_to_row(&mut self, row: usize) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let row = row.min(len - 1);
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor = row;
            return;
        }
        let was_sources = self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor);
        *self.active_cursor_mut() = row;
        if self.tab == Tab::Config && was_sources && !self.is_sources_list_block(row) {
            self.config_in_subnav = false;
        }
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.cursor = 0;
        self.config_in_subnav = false;
        self.editing = None;
        if self.tab == Tab::Logs {
            self.sync_log_cursor();
        }
    }

    pub fn sync_log_cursor(&mut self) {
        if self.tab != Tab::Logs {
            return;
        }
        let len = super::log_len();
        if len == 0 {
            self.logs_cursor = 0;
            self.logs_seen_len = 0;
            return;
        }
        if self.logs_cursor == 0 {
            self.logs_seen_len = len;
            return;
        }
        if len > self.logs_seen_len {
            self.logs_cursor = (self.logs_cursor + (len - self.logs_seen_len)).min(len - 1);
        } else if self.logs_cursor >= len {
            self.logs_cursor = len - 1;
        }
        self.logs_seen_len = len;
    }

    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Config => {
                if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
                    self.sources_subnav_len()
                } else {
                    Self::config_block_count()
                }
            }
            Tab::History => self.ctx.state.history.len(),
            Tab::Browse => self.browse_items().len(),
            Tab::Search => self.search_results.len(),
            Tab::Logs => super::log_len(),
            _ => 0,
        }
    }

    pub fn config_block_count() -> usize {
        5
    }

    fn active_cursor_mut(&mut self) -> &mut usize {
        match self.tab {
            Tab::Config => &mut self.config_cursor,
            Tab::Logs => &mut self.logs_cursor,
            _ => &mut self.cursor,
        }
    }

    pub fn history_lines(&self) -> Vec<String> {
        history_browse_view::history_lines(&self.ctx.state.history, self.cursor)
    }

    pub fn selected_history_preview_path(&self) -> Option<PathBuf> {
        history_browse_view::selected_history_preview_path(&self.ctx.state.history, self.cursor)
    }

    pub fn browse_lines(&self) -> Vec<String> {
        history_browse_view::browse_lines(self.browse_items(), self.cursor)
    }

    pub fn search_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("provider: Wallhaven | query: {}", self.search_query),
            format!(
                "filters: purity {} | categories {} | sorting {} {} | minimum {}",
                self.wallhaven_summary.purity,
                self.wallhaven_summary.categories,
                self.wallhaven_summary.sorting,
                self.wallhaven_summary.order,
                self.wallhaven_summary.atleast
            ),
        ];
        if self.search_results.is_empty() {
            lines.push(style::state_text(
                StateKind::Empty,
                "no results; press / or i to edit query, Enter to search",
            ));
        } else {
            for (i, hit) in self.search_results.iter().enumerate() {
                let mark = if i == self.cursor { ">" } else { " " };
                lines.push(format!("{mark} Wallhaven {} — {}", hit.id, hit.label));
            }
        }
        lines
    }

    pub fn logs_lines(&self, width: u16, height: u16) -> Vec<String> {
        let logs = super::LOG_BUFFER.lock().unwrap();
        if logs.is_empty() {
            return vec![style::state_text(StateKind::Empty, "no logs captured yet")];
        }
        let wrap_width = usize::from(width).saturating_sub(4);
        let mut lines = Vec::new();
        let mut selected_row = 0;
        for (i, line) in logs.iter().rev().enumerate() {
            let selected = i == self.logs_cursor;
            let mark = if selected { ">" } else { " " };
            let wrapped = wrap_log_text(line, wrap_width);
            for (j, segment) in wrapped.into_iter().enumerate() {
                if j == 0 {
                    if selected {
                        selected_row = lines.len();
                    }
                    lines.push(format!("{mark} {segment}"));
                } else {
                    lines.push(format!("  {segment}"));
                }
            }
        }
        crop_lines_around_selection(lines, selected_row, height)
    }

    pub fn browse_items(&self) -> Vec<String> {
        history_browse_view::browse_items(
            &self.ctx.state.cache_queue,
            &self.local_candidates,
            &self.ctx.state.history,
        )
    }

    pub fn selected_browse_preview_path(&self) -> Option<PathBuf> {
        history_browse_view::selected_browse_preview_path(
            self.browse_items(),
            self.cursor,
            &self.ctx.paths.cache_dir,
        )
    }

    pub fn selected_search_preview_path(&self) -> Option<PathBuf> {
        let hit = self.search_results.get(self.cursor)?;
        walls_core::wallhaven::cached_wallpaper_path(&self.ctx.paths.cache_dir, &hit.id)
    }

    pub fn apply_history_selection(&mut self) -> Option<PathBuf> {
        let path = self.ctx.state.history.get(self.cursor)?.clone();
        let p = PathBuf::from(&path);
        if p.exists() {
            self.ctx.apply_file(&p, ApplyTrigger::Manual).ok()?;
            return Some(p);
        }
        None
    }

    pub async fn apply_browse_selection(&mut self) -> anyhow::Result<Option<String>> {
        let items = self.browse_items();
        let Some(line) = items.get(self.cursor) else {
            return Ok(None);
        };
        if let Some(id) = line.strip_prefix("queue: ") {
            self.ctx.prioritize_cache_id(id)?;
            if let Some(p) = self.ctx.advance_next_manual().await? {
                return Ok(Some(format!("applied queue head: {}", p.display())));
            }
            return Ok(Some("queue item not applicable".into()));
        }
        if let Some(path) = line.strip_prefix("local: ") {
            let p = PathBuf::from(path);
            if p.exists() {
                self.ctx.apply_file(&p, ApplyTrigger::Manual)?;
                return Ok(Some(format!("applied: {}", p.display())));
            }
        }
        if let Some(path) = line.strip_prefix("history: ") {
            let p = PathBuf::from(path);
            if p.exists() {
                self.ctx.apply_file(&p, ApplyTrigger::Manual)?;
                return Ok(Some(format!("applied: {}", p.display())));
            }
        }
        Ok(None)
    }

    pub async fn run_search(&mut self) -> anyhow::Result<()> {
        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let mut params = self.ctx.config.wallhaven.search.clone();
        params.q = self.search_query.clone();
        let resp = client.search(&params, 1).await?;
        self.search_results = resp
            .data
            .into_iter()
            .map(|wp| SearchHit {
                id: wp.id.clone(),
                label: wp.path.clone(),
            })
            .collect();
        self.cursor = 0;
        Ok(())
    }

    pub async fn apply_search_selection(&mut self) -> anyhow::Result<Option<String>> {
        let Some(hit) = self.search_results.get(self.cursor) else {
            return Ok(None);
        };
        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let wp = client.fetch_wallpaper(&hit.id).await?;
        let path = client
            .download_to_cache_with_quota(
                &wp,
                &self.ctx.paths.cache_dir,
                &self.ctx.paths.download_dir,
                self.ctx.config.quota.size_mb,
                self.ctx.config.quota.enabled,
            )
            .await?;
        self.ctx.apply_file(&path, ApplyTrigger::Manual)?;
        Ok(Some(format!(
            "applied wallhaven-{}: {}",
            hit.id,
            path.display()
        )))
    }

    pub fn favorite_current(&mut self) -> anyhow::Result<String> {
        let dest = self.ctx.favorite_current()?;
        Ok(format!("favorited: {}", dest.display()))
    }

    pub fn trash_current(&mut self) -> anyhow::Result<String> {
        self.ctx.trash_current()?;
        Ok("trashed current wallpaper".into())
    }

    pub fn trash_current_prompt(&self) -> String {
        match self.ctx.plan_trash_current() {
            Ok(plan) => {
                let composed = plan
                    .composed_path
                    .as_ref()
                    .map(|path| format!(" + composed {path}"))
                    .unwrap_or_default();
                format!(
                    "trash: current wallpaper original {}{}? d confirm, Esc cancel",
                    plan.original_path, composed
                )
            }
            Err(e) => format!("trash: {e}"),
        }
    }

    pub fn nuke_downloads_prompt(&self) -> String {
        let plan = self.ctx.plan_nuke_downloads();
        match plan.mode {
            walls_core::downloads::NukeDownloadsMode::ClearQueue => format!(
                "provider queue: clear {} queued provider item{}? Shift+X confirm, Esc cancel",
                plan.queue_len,
                if plan.queue_len == 1 { "" } else { "s" }
            ),
            walls_core::downloads::NukeDownloadsMode::PurgeProviderFiles => format!(
                "provider files: delete {} cache + {} downloaded provider file{}? Shift+X confirm, Esc cancel",
                plan.cache_files,
                plan.download_files,
                if plan.cache_files + plan.download_files == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            walls_core::downloads::NukeDownloadsMode::ProviderReset => format!(
                "provider reset: clear {} queued, delete {} cache + {} downloaded file{}, prune {} history entr{}, current={}? Shift+X confirm, Esc cancel",
                plan.queue_len,
                plan.cache_files,
                plan.download_files,
                if plan.cache_files + plan.download_files == 1 {
                    ""
                } else {
                    "s"
                },
                plan.history_provider_entries,
                if plan.history_provider_entries == 1 { "y" } else { "ies" },
                plan.current_provider_storage
            ),
            walls_core::downloads::NukeDownloadsMode::Nothing => {
                "provider reset: no queue, cache, downloads, history, or current state to clear"
                    .into()
            }
        }
    }

    pub fn nuke_downloads(&mut self) -> anyhow::Result<String> {
        let result = self.ctx.nuke_downloads()?;
        Ok(match result.mode {
            walls_core::downloads::NukeDownloadsMode::ClearQueue => format!(
                "provider queue: cleared {} queued provider item{}",
                result.queue_cleared,
                if result.queue_cleared == 1 { "" } else { "s" }
            ),
            walls_core::downloads::NukeDownloadsMode::PurgeProviderFiles => format!(
                "provider files: removed {} cache + {} downloaded provider file{}",
                result.cache_removed,
                result.download_removed,
                if result.cache_removed + result.download_removed == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            walls_core::downloads::NukeDownloadsMode::ProviderReset => format!(
                "provider reset: cleared {} queued, removed {} cache + {} downloaded file{}, pruned {} history entr{}, current={}",
                result.queue_cleared,
                result.cache_removed,
                result.download_removed,
                if result.cache_removed + result.download_removed == 1 {
                    ""
                } else {
                    "s"
                },
                result.history_pruned,
                if result.history_pruned == 1 { "y" } else { "ies" },
                result.current_cleared
            ),
            walls_core::downloads::NukeDownloadsMode::Nothing => {
                "provider reset: no queue, cache, downloads, history, or current state to clear"
                    .into()
            }
        })
    }

    pub fn toggle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
            let idx = self.config_sub_cursor;
            if self.is_wallhaven_subnav_index(idx) {
                config.wallhaven.enabled = !config.wallhaven.enabled;
                persist_config(&self.ctx.paths.config_file, &config)?;
                return Ok(Some(format!(
                    "config saved: wallhaven enabled={}",
                    config.wallhaven.enabled
                )));
            }
            if idx < config.sources.len() {
                let label = config.sources[idx]
                    .label
                    .clone()
                    .unwrap_or_else(|| config.sources[idx].source_type.clone());
                config.sources[idx].enabled = !config.sources[idx].enabled;
                persist_config(&self.ctx.paths.config_file, &config)?;
                return Ok(Some(format!(
                    "config saved: {label} enabled={}",
                    config.sources[idx].enabled
                )));
            }
        }
        let message = match self.config_cursor {
            0 => {
                config.change.enabled = !config.change.enabled;
                format!("config saved: rotation enabled={}", config.change.enabled)
            }
            2 => {
                config.quota.enabled = !config.quota.enabled;
                format!("config saved: quota enabled={}", config.quota.enabled)
            }
            3 => {
                config.display.auto_rotate = !config.display.auto_rotate;
                format!("config saved: auto rotate={}", config.display.auto_rotate)
            }
            _ => return Ok(None),
        };

        persist_config(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    pub fn cycle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        let message = match self.config_cursor {
            2 => {
                config.selection.strategy = match config.selection.strategy {
                    SelectionStrategy::Random => SelectionStrategy::Sequential,
                    SelectionStrategy::Sequential => SelectionStrategy::Random,
                };
                format!("config saved: selection={:?}", config.selection.strategy)
            }
            _ => return Ok(None),
        };

        persist_config(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    #[allow(dead_code)]
    pub fn start_edit_for_current(&mut self) {
        if self.tab != Tab::Config {
            return;
        }
        let target = if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
            self.selected_sources_subnav_edit_target()
        } else if self.is_sources_list_block(self.config_cursor) {
            match self.default_sources_edit_target() {
                Some(target) => Some(target),
                None => {
                    self.set_message(
                        StatusKind::Warning,
                        "no active sources to edit; enable a source or Wallhaven first",
                    );
                    None
                }
            }
        } else {
            Some(EditTarget::Block(self.config_cursor))
        };

        let Some(target) = target else {
            return;
        };
        let Some(session) = self.edit_session_for_target(target) else {
            return;
        };
        self.editing = Some(session);
        let new_buf = self.current_edit_field_value();
        if let Some(s) = &mut self.editing {
            s.field_buffer = new_buf;
        }
        self.clear_message();
    }

    fn selected_sources_subnav_edit_target(&self) -> Option<EditTarget> {
        let idx = self.config_sub_cursor;
        if self.is_wallhaven_subnav_index(idx) {
            Some(EditTarget::Wallhaven)
        } else if idx < self.ctx.config.sources.len() {
            Some(EditTarget::Source(idx))
        } else {
            None
        }
    }

    fn default_sources_edit_target(&self) -> Option<EditTarget> {
        self.ctx
            .config
            .sources
            .iter()
            .position(|source| source.enabled)
            .map(EditTarget::Source)
            .or_else(|| {
                self.ctx
                    .config
                    .wallhaven
                    .enabled
                    .then_some(EditTarget::Wallhaven)
            })
    }

    fn edit_session_for_target(&self, target: EditTarget) -> Option<EditSession> {
        match &target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let idx = *i;
                let mut draft = self.ctx.config.sources[idx].clone();
                normalize_source_entry(&mut draft);
                Some(EditSession {
                    target: target.clone(),
                    draft_source: Some(draft),
                    draft_block_values: std::collections::HashMap::new(),
                    field_cursor: 0,
                    field_buffer: String::new(),
                    validation_errors: vec![],
                })
            }
            EditTarget::Block(0) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: rotation_block_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Block(2) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: library_block_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Block(4) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: tui_block_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Wallhaven => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: wallhaven_block_draft(
                    &self.ctx.config,
                    wallhaven_api_key_present(&self.ctx.secrets),
                ),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.set_message(StatusKind::Neutral, "edit cancelled");
    }

    #[allow(dead_code)]
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub(crate) fn edit_field_count(&self) -> usize {
        let Some(sess) = &self.editing else {
            return 0;
        };
        match &sess.target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let src = sess
                    .draft_source
                    .as_ref()
                    .unwrap_or(&self.ctx.config.sources[*i]);
                Self::source_editable_fields(src).len()
            }
            EditTarget::Block(0) => ROTATION_BLOCK_FIELDS.len(),
            EditTarget::Block(2) => LIBRARY_BLOCK_FIELDS.len(),
            EditTarget::Block(4) => TUI_BLOCK_FIELDS.len(),
            EditTarget::Block(_) => 0,
            EditTarget::Wallhaven => WALLHAVEN_BLOCK_FIELDS.len(),
            _ => 0,
        }
    }

    pub(crate) fn current_edit_field_kind(&self) -> EditFieldKind {
        let Some(sess) = &self.editing else {
            return EditFieldKind::Text;
        };
        match &sess.target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let src = sess
                    .draft_source
                    .as_ref()
                    .unwrap_or(&self.ctx.config.sources[*i]);
                let names = Self::source_editable_fields(src);
                if sess.field_cursor < names.len() {
                    source_field_kind_for(src, &names[sess.field_cursor])
                } else {
                    EditFieldKind::Text
                }
            }
            EditTarget::Block(block) => {
                let keys = match block {
                    0 => ROTATION_BLOCK_FIELDS,
                    2 => LIBRARY_BLOCK_FIELDS,
                    4 => TUI_BLOCK_FIELDS,
                    _ => &[] as &[&str],
                };
                if let Some(key) = keys.get(sess.field_cursor) {
                    block_field_kind(*block, key)
                } else {
                    EditFieldKind::Text
                }
            }
            EditTarget::Wallhaven => {
                if let Some(key) = WALLHAVEN_BLOCK_FIELDS.get(sess.field_cursor) {
                    block_field_kind(WALLHAVEN_FIELDS_BLOCK, key)
                } else {
                    EditFieldKind::Text
                }
            }
            _ => EditFieldKind::Text,
        }
    }

    pub(crate) fn wallhaven_block_field_locked(&self, key: &str) -> bool {
        key == "purity_nsfw" && !wallhaven_api_key_present(&self.ctx.secrets)
    }

    pub(crate) fn current_edit_field_locked(&self) -> bool {
        let Some(sess) = &self.editing else {
            return false;
        };
        if let EditTarget::Wallhaven = &sess.target {
            if let Some(key) = WALLHAVEN_BLOCK_FIELDS.get(sess.field_cursor) {
                return self.wallhaven_block_field_locked(key);
            }
        }
        if let EditTarget::Source(_) = &sess.target {
            if let Some(draft) = &sess.draft_source {
                let names = Self::source_editable_fields(draft);
                if let Some(name) = names.get(sess.field_cursor) {
                    return name == "time" && reddit_time_field_locked(draft);
                }
            }
        }
        false
    }

    pub(crate) fn reddit_field_display_value(
        &self,
        src: &SourceEntry,
        key: &str,
        value: &str,
        kind: EditFieldKind,
    ) -> String {
        if key == "time" && reddit_time_field_locked(src) {
            return "n/a (top/controversial only)".into();
        }
        Self::choice_display_for_current_field(value, kind)
    }

    pub(crate) fn wallhaven_field_display_value(
        &self,
        key: &str,
        value: &str,
        kind: EditFieldKind,
    ) -> String {
        if self.wallhaven_block_field_locked(key) {
            return "unavailable (no API key)".into();
        }
        choice_display_value(kind, value)
    }

    pub(crate) fn cycle_current_edit_field(&mut self, forward: bool) {
        if self.current_edit_field_locked() {
            return;
        }
        let kind = self.current_edit_field_kind();
        let next = match kind {
            EditFieldKind::Text => return,
            EditFieldKind::Bool => {
                let current = self
                    .editing
                    .as_ref()
                    .map(|s| s.field_buffer.clone())
                    .unwrap_or_default();
                toggle_bool_value(&current)
            }
            EditFieldKind::Choice(options) => {
                let current = self
                    .editing
                    .as_ref()
                    .map(|s| s.field_buffer.clone())
                    .unwrap_or_default();
                cycle_choice_value(&current, options, forward)
            }
        };
        if let Some(sess) = &mut self.editing {
            sess.field_buffer = next;
        }
        self.commit_edit_field_buffer();
        let _ = self.save_edit_item(false);
        let refreshed = self.current_edit_field_value();
        if let Some(sess) = &mut self.editing {
            sess.field_buffer = refreshed;
        }
        self.refresh_edit_validation();
    }

    pub(crate) fn choice_display_for_current_field(value: &str, kind: EditFieldKind) -> String {
        choice_display_value(kind, value)
    }

    #[allow(dead_code)]
    pub fn source_editable_fields(src: &walls_core::config::SourceEntry) -> Vec<String> {
        core_source_editable_fields(src)
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    #[allow(dead_code)]
    pub fn get_source_field(src: &walls_core::config::SourceEntry, name: &str) -> String {
        match name {
            "enabled" => src.enabled.to_string(),
            "type" => src.source_type.clone(),
            "label" => src.label.clone().unwrap_or_default(),
            "url" => src.url.clone().unwrap_or_default(),
            "path" => src.path.clone().unwrap_or_default(),
            "image_path" => src.image_path.clone().unwrap_or_default(),
            "query" => src.query.clone().unwrap_or_default(),
            "api_key" => src.api_key.clone().unwrap_or_default(),
            "collection" => src.collection.clone().unwrap_or_default(),
            "user" => src.user.clone().unwrap_or_default(),
            "topic" => src.topic.clone().unwrap_or_default(),
            "orientation" => src.orientation.clone().unwrap_or_default(),
            "sort" => reddit_sort_value(src).to_string(),
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
    /// Prevents "I set false, s just fails" from strict parse only accepting "true"/"false".
    fn parse_bool_like(s: &str) -> Option<bool> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "true" | "t" | "1" | "yes" | "y" | "on" => Some(true),
            "false" | "f" | "0" | "no" | "n" | "off" => Some(false),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn set_source_field(draft: &mut walls_core::config::SourceEntry, name: &str, buf: &str) {
        let trimmed = buf.trim();
        let v = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        match name {
            "enabled" => {
                draft.enabled = Self::parse_bool_like(trimmed).unwrap_or(draft.enabled);
            }
            "type" => {
                draft.source_type = trimmed.to_string();
            }
            "label" => draft.label = v,
            "url" => draft.url = v,
            "path" => draft.path = v,
            "image_path" => draft.image_path = v,
            "query" => draft.query = v,
            "api_key" => draft.api_key = v,
            "collection" => draft.collection = v,
            "user" => draft.user = v,
            "topic" => draft.topic = v,
            "orientation" => draft.orientation = v,
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

    /// Pure value lookup for a field at a given cursor idx for a target (no reliance on live editing sess cursor).
    /// Used by up/down handlers to precompute the *new* position's buffer value without borrow conflicts.
    #[allow(dead_code)]
    pub fn edit_field_value_at(&self, target: &EditTarget, idx: usize) -> String {
        match target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                // Prefer draft for the matching editing target (so nav in edit after commits prefills
                // edited values from draft state that originated from json config).
                let src = if let Some(sess) = &self.editing {
                    if let Some(d) = &sess.draft_source {
                        if matches!(&sess.target, EditTarget::Source(j) if j == i) {
                            d
                        } else {
                            &self.ctx.config.sources[*i]
                        }
                    } else {
                        &self.ctx.config.sources[*i]
                    }
                } else {
                    &self.ctx.config.sources[*i]
                };
                let names = Self::source_editable_fields(src);
                if idx < names.len() {
                    Self::get_source_field(src, &names[idx])
                } else {
                    String::new()
                }
            }
            EditTarget::Block(block) => {
                let draft = self
                    .editing
                    .as_ref()
                    .map(|sess| sess.draft_block_values.clone())
                    .unwrap_or_default();
                block_field_value_at(&self.ctx.config, *block, &draft, idx)
            }
            EditTarget::Wallhaven => {
                let draft = self
                    .editing
                    .as_ref()
                    .map(|sess| sess.draft_block_values.clone())
                    .unwrap_or_default();
                block_field_value_at(&self.ctx.config, WALLHAVEN_FIELDS_BLOCK, &draft, idx)
            }
            _ => String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn current_edit_field_value(&self) -> String {
        if let Some(sess) = &self.editing {
            let idx = sess.field_cursor;
            match &sess.target {
                EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                    // Prefer draft (the in-memory edit copy started from the json config at e) so that
                    // after field commits (which update only draft), moving fields prefills from the
                    // "current config item state" not stale live ctx. This makes prefill reflect the
                    // json + uncommitted edits.
                    let src = if let Some(d) = &sess.draft_source {
                        d
                    } else {
                        &self.ctx.config.sources[*i]
                    };
                    let names = Self::source_editable_fields(src);
                    if idx < names.len() {
                        return Self::get_source_field(src, &names[idx]);
                    }
                    String::new()
                }
                EditTarget::Block(block) => {
                    block_field_value_at(&self.ctx.config, *block, &sess.draft_block_values, idx)
                }
                EditTarget::Wallhaven => block_field_value_at(
                    &self.ctx.config,
                    WALLHAVEN_FIELDS_BLOCK,
                    &sess.draft_block_values,
                    idx,
                ),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    }

    pub fn is_sources_list_block(&self, block: usize) -> bool {
        block == 1 // the "Sources" block (was Local sources)
    }

    fn validation_issues_for_edit(
        target: &EditTarget,
        config: &Config,
        secrets: &walls_core::config::Secrets,
        paths: &walls_core::paths::WallsPaths,
    ) -> Vec<String> {
        match target {
            EditTarget::Source(i) => validate_source_edit(*i, config, secrets, paths),
            EditTarget::Wallhaven => validate_wallhaven_edit(config, secrets),
            EditTarget::Block(2) => validate_config_diagnostics(config, secrets, paths)
                .into_iter()
                .filter(|diagnostic| diagnostic.path.starts_with("quota."))
                .map(|diagnostic| diagnostic.to_string())
                .collect(),
            EditTarget::Block(_) => Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn toggle_config_subnav(&mut self) {
        if self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor) {
            self.config_in_subnav = !self.config_in_subnav;
            if self.config_in_subnav {
                self.config_sub_cursor = 0;
            }
        }
    }

    #[allow(dead_code)]
    pub fn enter_config_subnav(&mut self) {
        if self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor) {
            self.config_in_subnav = true;
            self.config_sub_cursor = 0;
        }
    }

    pub fn exit_config_subnav(&mut self) {
        self.config_in_subnav = false;
    }

    #[allow(dead_code)]
    pub fn refresh_edit_validation(&mut self) {
        if let Some(sess) = &mut self.editing {
            // Build a temp view of the item and validate relevant parts
            // For simplicity, clone full config, patch, run validate, filter
            let mut temp = self.ctx.config.clone();
            match &sess.target {
                EditTarget::Source(i) if *i < temp.sources.len() => {
                    if let Some(d) = &sess.draft_source {
                        temp.sources[*i] = d.clone();
                    }
                }
                EditTarget::Block(0) => {
                    apply_rotation_block_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Block(2) => {
                    apply_library_block_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Block(4) => {
                    apply_tui_block_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Wallhaven => {
                    apply_wallhaven_block_draft(
                        &mut temp,
                        &sess.draft_block_values,
                        wallhaven_api_key_present(&self.ctx.secrets),
                    );
                }
                _ => {}
            }
            let issues = Self::validation_issues_for_edit(
                &sess.target,
                &temp,
                &self.ctx.secrets,
                &self.ctx.paths,
            );
            // keep only issues mentioning the target roughly
            sess.validation_errors = issues
                .into_iter()
                .filter(|e| match &sess.target {
                    EditTarget::Source(_) => {
                        e.contains("source")
                            || e.contains("path")
                            || e.contains("url")
                            || e.contains("key")
                            || e.contains("secrets")
                            || e.contains("unsplash")
                            || e.contains("reddit")
                    }
                    EditTarget::Wallhaven => {
                        e.contains("wallhaven") || e.contains("NSFW") || e.contains("purity")
                    }
                    _ => true,
                })
                .collect();
        }
    }

    #[allow(dead_code)]
    pub fn commit_edit_field_buffer(&mut self) {
        if let Some(sess) = &mut self.editing {
            let buf = std::mem::take(&mut sess.field_buffer);
            let field_idx = sess.field_cursor;
            match &mut sess.target {
                EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                    if let Some(draft) = &mut sess.draft_source {
                        let names = Self::source_editable_fields(draft);
                        if field_idx < names.len() {
                            let name = &names[field_idx];
                            let prev_type = draft.source_type.clone();
                            Self::set_source_field(draft, name, &buf);
                            if name == "type" && draft.source_type != prev_type {
                                normalize_source_entry(draft);
                            }
                        }
                    }
                }
                EditTarget::Block(block) => {
                    commit_block_field_buffer(
                        *block,
                        field_idx,
                        &buf,
                        &mut sess.draft_block_values,
                    );
                }
                EditTarget::Wallhaven => {
                    commit_block_field_buffer(
                        WALLHAVEN_FIELDS_BLOCK,
                        field_idx,
                        &buf,
                        &mut sess.draft_block_values,
                    );
                }
                _ => {}
            }
            self.refresh_edit_validation();
        }
    }

    #[allow(dead_code)]
    pub fn save_edit_item(&mut self, exit_on_success: bool) -> anyhow::Result<()> {
        // Auto-commit only if there's a pending non-empty buffer (e.g. direct Save action use, or 's' if ever mapped).
        // When caller did explicit commit first (Enter or arrow move), buffer is empty so we avoid re-committing empty
        // which would clear text fields.
        if self.editing.is_some() {
            let has_pending = if let Some(s) = &self.editing {
                !s.field_buffer.is_empty()
            } else {
                false
            };
            if has_pending {
                self.commit_edit_field_buffer();
            }
        }
        let sess = match &self.editing {
            Some(s) => s,
            None => return Ok(()),
        };
        let mut config = self.ctx.config.clone();
        let mut success_msg = "config saved via edit".to_string();
        match &sess.target {
            EditTarget::Source(i) if *i < config.sources.len() => {
                if let Some(d) = &sess.draft_source {
                    let mut saved = d.clone();
                    normalize_source_entry(&mut saved);
                    config.sources[*i] = saved;
                    success_msg = if d.source_type == "reddit" {
                        format!("config saved: reddit source #{i}")
                    } else {
                        format!("config saved: source #{} type={}", i, d.source_type)
                    };
                }
            }
            EditTarget::Block(0) => {
                apply_rotation_block_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: rotation".into();
            }
            EditTarget::Block(2) => {
                apply_library_block_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: library".into();
            }
            EditTarget::Block(4) => {
                apply_tui_block_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: tui preferences".into();
            }
            EditTarget::Wallhaven => {
                apply_wallhaven_block_draft(
                    &mut config,
                    &sess.draft_block_values,
                    wallhaven_api_key_present(&self.ctx.secrets),
                );
                success_msg = "config saved: wallhaven".into();
            }
            _ => {}
        }
        let issues = Self::validation_issues_for_edit(
            &sess.target,
            &config,
            &self.ctx.secrets,
            &self.ctx.paths,
        );
        if !issues.is_empty() {
            if let Some(s) = &mut self.editing {
                s.validation_errors = issues.clone();
            }
            self.set_message(
                StatusKind::Error,
                format!("config validation failed: {}", issues.join("; ")),
            );
            return Ok(());
        }
        persist_config(&self.ctx.paths.config_file, &config)?;
        self.set_message(StatusKind::Success, success_msg);
        // reload will happen via effect if we return it, but for simplicity here reload
        self.reload_ctx()?;
        if exit_on_success {
            self.editing = None;
        }
        Ok(())
    }

    pub(crate) fn complete_command(&mut self, forward: bool) {
        if let Some(next) = command::complete(&self.cmd_line, forward) {
            self.cmd_line.clear();
            self.cmd_line.push_str(next);
        }
    }

    pub fn run_command(
        &mut self,
        rt: &tokio::runtime::Handle,
    ) -> anyhow::Result<Option<(String, StatusKind)>> {
        let message = match ParsedCommand::parse(&self.cmd_line) {
            ParsedCommand::Next => {
                match tokio::task::block_in_place(|| rt.block_on(self.ctx.advance_next_manual())) {
                    Ok(Some(p)) => (format!("next: {}", p.display()), StatusKind::Success),
                    Ok(None) => (crate::recovery::tui_next_no_change(), StatusKind::Neutral),
                    Err(e) => (format!("next error: {e}"), StatusKind::Error),
                }
            }
            ParsedCommand::Prev => match self.ctx.advance_prev() {
                Ok(Some(p)) => (format!("prev: {}", p.display()), StatusKind::Success),
                Ok(None) => (crate::recovery::tui_no_previous(), StatusKind::Neutral),
                Err(e) => (format!("prev error: {e}"), StatusKind::Error),
            },
            ParsedCommand::TogglePause => {
                self.ctx.toggle_pause()?;
                (
                    format!("paused: {}", self.ctx.state.paused),
                    StatusKind::Success,
                )
            }
            ParsedCommand::Favorite => match self.favorite_current() {
                Ok(msg) => (msg, StatusKind::Success),
                Err(e) => (crate::recovery::favorite_error(&e), StatusKind::Error),
            },
            ParsedCommand::Status => (
                format!(
                    "paused={} history={} queue={}",
                    self.ctx.state.paused,
                    self.ctx.state.history.len(),
                    self.ctx.state.cache_queue.len()
                ),
                StatusKind::Neutral,
            ),
            ParsedCommand::Quit => return Ok(None),
            ParsedCommand::Empty => ("(empty command)".into(), StatusKind::Warning),
            ParsedCommand::Unknown(other) => (
                format!(
                    "unknown command: {other} (try :next :prev :pause :favorite :status :quit)"
                ),
                StatusKind::Error,
            ),
        };
        Ok(Some(message))
    }

    pub fn footer_keys(&self) -> String {
        if self.show_key_help {
            return "Key help | Esc/q close".into();
        }
        if self.is_editing() {
            let choice_hint = if self.current_edit_field_locked() {
                if self
                    .editing
                    .as_ref()
                    .and_then(|s| s.draft_source.as_ref())
                    .is_some_and(|src| src.source_type == "reddit")
                {
                    "top/controversial only"
                } else {
                    "requires API key"
                }
            } else {
                match self.current_edit_field_kind() {
                    EditFieldKind::Text => "type/Backspace",
                    EditFieldKind::Bool => "Space toggle",
                    EditFieldKind::Choice(_) => "Space/←/→ cycle",
                }
            };
            return format!("edit: ↑/↓ fields | {choice_hint} | Enter save | Esc cancel | q");
        }
        if self.pending_trash_confirm {
            return "d confirm trash current wallpaper | Esc cancel".into();
        }
        if self.pending_nuke_confirm {
            return "Shift+X confirm provider reset | Esc cancel".into();
        }
        let keys = match self.input_mode {
            InputMode::Command => {
                format!(
                    ":{}_ | Ctrl+n/p complete | Enter run Esc cancel",
                    self.cmd_line
                )
            }
            InputMode::SearchInput => "Search: type query | Enter search Esc cancel".to_string(),
            InputMode::Normal => match self.tab {
                Tab::Search => {
                    let enter_hint = if self.search_results.is_empty() {
                        "Enter search"
                    } else {
                        "Enter apply"
                    };
                    format!(
                        "{} | / or i edit query | {enter_hint} | j/k Pg Home/End | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
                Tab::Config => {
                    if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
                        format!(
                            "{} | Esc back | j/k Pg Home/End pick source | e edit | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT
                        )
                    } else if self.is_sources_list_block(self.config_cursor) {
                        format!(
                            "{} | j/k Pg Home/End | e first active | Enter pick | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT
                        )
                    } else {
                        format!(
                            "{} | j/k Pg Home/End | e edit | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT
                        )
                    }
                }
                Tab::Logs => {
                    format!(
                        "{} | newest first | j older k newer | Home newest End oldest | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
                _ => {
                    format!(
                        "{} | j/k Pg Home/End | n/p next/prev | f favorite d request trash | Shift+X reset | space pause | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
            },
        };
        format!("{keys} | q quit")
    }

    pub fn compact_footer_keys(&self) -> String {
        if self.show_key_help {
            return "help | Esc/q close".into();
        }
        if self.is_editing() {
            return "edit | ↑/↓ fields | Space/←/→ | Enter | Esc | q".into();
        }
        if self.pending_trash_confirm {
            return "d confirm trash | Esc cancel".into();
        }
        if self.pending_nuke_confirm {
            return "Shift+X confirm reset | Esc cancel".into();
        }
        match self.input_mode {
            InputMode::Command => format!(":{}_ | Enter | Esc | q", self.cmd_line),
            InputMode::SearchInput => "type | Enter search | Esc | q".into(),
            InputMode::Normal => {
                let nav = Self::NORMAL_TAB_NAV_HINT;
                match self.tab {
                    Tab::Search => {
                        let enter_hint = if self.search_results.is_empty() {
                            "Enter search"
                        } else {
                            "Enter apply"
                        };
                        format!("{nav} /i {enter_hint} j/k :?q")
                    }
                    Tab::Config
                        if self.config_in_subnav
                            && self.is_sources_list_block(self.config_cursor) =>
                    {
                        format!("{nav} Esc j/k Pg e t n/p sp :?q")
                    }
                    Tab::Config => {
                        format!("{nav} j/k Pg Enter e t n/p sp :?q")
                    }
                    Tab::Logs => format!("{nav} newest j older k newer :?q"),
                    _ => format!("{nav} j/k Pg n/p f/d? Shift+X sp :?q"),
                }
            }
        }
    }
}

fn is_local_source(source: &SourceEntry) -> bool {
    matches!(
        source.source_type.as_str(),
        "folder" | "image" | "favorites" | "fetched"
    )
}

fn summarize_local_source(ctx: &WallsCtx, source: &SourceEntry) -> LocalSourceSummary {
    let label = source
        .label
        .clone()
        .unwrap_or_else(|| source.source_type.clone());
    let path = match source.source_type.as_str() {
        "favorites" => Some(ctx.paths.favorites_dir.clone()),
        "fetched" => Some(ctx.paths.fetched_dir.clone()),
        "folder" | "image" => source.path.as_ref().map(expand_home),
        _ => None,
    };

    let Some(path) = path else {
        return LocalSourceSummary {
            enabled: source.enabled,
            source_type: source.source_type.clone(),
            label,
            path: "(not configured)".into(),
            status: "missing path".into(),
            candidates: 0,
        };
    };

    let path_status = if path.exists() {
        "ready"
    } else {
        "missing path"
    };
    let enabled_status = if source.enabled { "" } else { "disabled, " };
    let candidates =
        list_images_with_paths(source, &ctx.paths.favorites_dir, &ctx.paths.fetched_dir)
            .map_or(0, |images| images.len());

    LocalSourceSummary {
        enabled: source.enabled,
        source_type: source.source_type.clone(),
        label,
        path: path.display().to_string(),
        status: format!("{enabled_status}{path_status}"),
        candidates,
    }
}

fn summarize_wallhaven_provider(ctx: &WallsCtx) -> WallhavenProviderSummary {
    let search = &ctx.config.wallhaven.search;
    let api_key_present = !ctx.secrets.wallhaven_api_key.trim().is_empty();
    let query = if search.q.trim().is_empty() {
        "(empty query)".into()
    } else {
        search.q.clone()
    };
    let collections = ctx
        .config
        .wallhaven
        .collections
        .iter()
        .map(|collection| {
            let label = collection.label.as_deref().unwrap_or("collection");
            format!("{}: {}/{}", label, collection.username, collection.id)
        })
        .collect();

    let mut warnings = Vec::new();
    if !ctx.config.change.internet_enabled {
        warnings.push("warning: online sources disabled".into());
    }
    if !api_key_present {
        warnings.push("warning: API key missing; NSFW purity unavailable".into());
    }
    if search.purity.chars().nth(2) == Some('1') {
        warnings.push("warning: NSFW purity requires Wallhaven account access".into());
    }

    WallhavenProviderSummary {
        enabled: ctx.config.wallhaven.enabled,
        internet_enabled: ctx.config.change.internet_enabled,
        api_key_present,
        prefer: format!("{:?}", ctx.config.wallhaven.prefer),
        collections,
        query,
        categories: search.categories.clone(),
        purity: search.purity.clone(),
        sorting: search.sorting.clone(),
        order: search.order.clone(),
        atleast: search.atleast.clone(),
        warnings,
    }
}

fn summarize_config_warnings(ctx: &WallsCtx) -> Vec<String> {
    validate_config_diagnostics(&ctx.config, &ctx.secrets, &ctx.paths)
        .into_iter()
        .map(|diagnostic| {
            let mut warning = format!("warning: {}: {}", diagnostic.path, diagnostic.message);
            if let Some(hint) = diagnostic.hint {
                warning.push_str(&format!(" (hint: {hint})"));
            }
            warning
        })
        .collect()
}

fn wrap_log_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.len() <= width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let extra = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };
        if current.is_empty() {
            current = word.to_string();
        } else if extra <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

fn crop_lines_around_selection(
    lines: Vec<String>,
    selected_row: usize,
    viewport_height: u16,
) -> Vec<String> {
    let visible_rows = usize::from(viewport_height).saturating_sub(2).max(1);
    if lines.len() <= visible_rows {
        return lines;
    }
    let start = selected_row
        .saturating_add(1)
        .saturating_sub(visible_rows)
        .min(lines.len().saturating_sub(visible_rows));
    lines.into_iter().skip(start).take(visible_rows).collect()
}

#[cfg(test)]
mod tests {
    use super::Tab;

    #[test]
    fn tab_indices_round_trip_through_visible_order() {
        let tabs = [
            Tab::Config,
            Tab::Now,
            Tab::History,
            Tab::Browse,
            Tab::Search,
        ];

        for (index, tab) in tabs.into_iter().enumerate() {
            assert_eq!(tab.index(), index);
            assert_eq!(Tab::from_index(index), tab);
        }
    }

    #[test]
    fn unknown_tab_index_falls_back_to_config() {
        assert_eq!(Tab::from_index(usize::MAX), Tab::Config);
    }
}

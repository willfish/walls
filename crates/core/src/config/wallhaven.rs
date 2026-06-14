use serde::{Deserialize, Serialize};
use std::process::Command;

use super::SourceEntry;

pub const WALLHAVEN_RESOLUTION_CHOICES: &[&str] = &[
    "1024x768",
    "1280x720",
    "1366x768",
    "1600x900",
    "1920x1080",
    "2560x1440",
    "3440x1440",
    "3840x2160",
];

pub const WALLHAVEN_RATIO_CHOICES: &[&str] = &[
    "16x9", "16x10", "21x9", "32x9", "48x9", "9x16", "10x16", "9x18", "1x1", "3x2", "4x3", "5x4",
];

pub const WALLHAVEN_FALLBACK_RESOLUTION: &str = "1920x1080";
pub const WALLHAVEN_FALLBACK_RATIO: &str = "16x9";
pub const WALLHAVEN_DEFAULT_QUERY: &str = "space";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenCollection {
    pub username: String,
    pub id: u32,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenSearch {
    #[serde(default = "default_query")]
    pub q: String,
    #[serde(default = "default_categories")]
    pub categories: String,
    #[serde(default = "default_purity")]
    pub purity: String,
    #[serde(default = "default_sorting")]
    pub sorting: String,
    #[serde(default = "default_order")]
    pub order: String,
    #[serde(default = "default_atleast")]
    pub atleast: String,
    #[serde(default = "default_ratios")]
    pub ratios: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WallhavenPrefer {
    CollectionsThenSearch,
    SearchOnly,
    CollectionsOnly,
}

impl Default for WallhavenPrefer {
    fn default() -> Self {
        default_prefer()
    }
}

impl Default for WallhavenSearch {
    fn default() -> Self {
        Self {
            q: WALLHAVEN_DEFAULT_QUERY.into(),
            categories: default_categories(),
            purity: default_purity(),
            sorting: default_sorting(),
            order: default_order(),
            atleast: default_atleast(),
            ratios: default_ratios(),
        }
    }
}

fn default_prefer() -> WallhavenPrefer {
    WallhavenPrefer::CollectionsThenSearch
}
fn default_query() -> String {
    WALLHAVEN_DEFAULT_QUERY.into()
}
fn default_categories() -> String {
    "111".into()
}
fn default_purity() -> String {
    "100".into()
}
fn default_sorting() -> String {
    "random".into()
}
fn default_order() -> String {
    "desc".into()
}
fn default_atleast() -> String {
    detected_wallhaven_search_defaults().atleast
}
fn default_ratios() -> String {
    detected_wallhaven_search_defaults().ratios
}

pub fn detected_wallhaven_search_defaults() -> WallhavenSearchDefaults {
    match main_monitor_resolution() {
        Some((width, height)) => WallhavenSearchDefaults {
            atleast: wallhaven_atleast_for_monitor(width, height).into(),
            ratios: wallhaven_ratio_for_monitor(width, height).into(),
        },
        None => WallhavenSearchDefaults {
            atleast: WALLHAVEN_FALLBACK_RESOLUTION.into(),
            ratios: WALLHAVEN_FALLBACK_RATIO.into(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallhavenSearchDefaults {
    pub atleast: String,
    pub ratios: String,
}

pub fn wallhaven_resolution_choices() -> &'static [&'static str] {
    WALLHAVEN_RESOLUTION_CHOICES
}

pub fn wallhaven_ratio_choices() -> &'static [&'static str] {
    WALLHAVEN_RATIO_CHOICES
}

pub fn wallhaven_resolution_supported(value: &str) -> bool {
    WALLHAVEN_RESOLUTION_CHOICES.contains(&value)
}

pub fn wallhaven_ratio_supported(value: &str) -> bool {
    WALLHAVEN_RATIO_CHOICES.contains(&value)
}

pub fn wallhaven_atleast_for_monitor(width: u32, height: u32) -> &'static str {
    WALLHAVEN_RESOLUTION_CHOICES
        .iter()
        .rev()
        .copied()
        .find(|choice| {
            parse_resolution(choice).is_some_and(|(choice_width, choice_height)| {
                choice_width <= width && choice_height <= height
            })
        })
        .unwrap_or(WALLHAVEN_FALLBACK_RESOLUTION)
}

pub fn wallhaven_ratio_for_monitor(width: u32, height: u32) -> &'static str {
    if width == 0 || height == 0 {
        return WALLHAVEN_FALLBACK_RATIO;
    }
    let monitor_ratio = f64::from(width) / f64::from(height);
    WALLHAVEN_RATIO_CHOICES
        .iter()
        .copied()
        .min_by(|a, b| {
            let a_delta = ratio_delta(monitor_ratio, a);
            let b_delta = ratio_delta(monitor_ratio, b);
            a_delta.total_cmp(&b_delta)
        })
        .unwrap_or(WALLHAVEN_FALLBACK_RATIO)
}

pub fn default_wallhaven_source() -> SourceEntry {
    let search = WallhavenSearch::default();
    SourceEntry {
        enabled: true,
        source_type: "wallhaven".into(),
        label: None,
        path: None,
        query: Some(search.q),
        url: None,
        collection: None,
        user: None,
        topic: None,
        orientation: None,
        api_key: None,
        image_path: None,
        title_path: None,
        source: None,
        author: None,
        sort: None,
        time: None,
        categories: Some(search.categories),
        purity: Some(search.purity),
        sorting: Some(search.sorting),
        order: Some(search.order),
        atleast: Some(search.atleast),
        ratios: Some(search.ratios),
        broaden_when_cache_below: None,
        prefer: Some(default_prefer()),
        collections: Vec::new(),
    }
}

pub fn populate_wallhaven_source_defaults(source: &mut SourceEntry) {
    let defaults = WallhavenSearch::default();
    if source
        .categories
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.categories = Some(defaults.categories);
    }
    if source
        .purity
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.purity = Some(defaults.purity);
    }
    if source
        .sorting
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.sorting = Some(defaults.sorting);
    }
    if source
        .order
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.order = Some(defaults.order);
    }
    if source
        .atleast
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.atleast = Some(defaults.atleast);
    }
    if source
        .ratios
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        source.ratios = Some(defaults.ratios);
    }
    if source.prefer.is_none() {
        source.prefer = Some(default_prefer());
    }
}

pub fn source_wallhaven_search(source: &SourceEntry) -> WallhavenSearch {
    let defaults = WallhavenSearch::default();
    WallhavenSearch {
        q: source
            .query
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        categories: source
            .categories
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.categories)
            .to_string(),
        purity: source
            .purity
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.purity)
            .to_string(),
        sorting: source
            .sorting
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.sorting)
            .to_string(),
        order: source
            .order
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.order)
            .to_string(),
        atleast: source
            .atleast
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.atleast)
            .to_string(),
        ratios: source
            .ratios
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&defaults.ratios)
            .to_string(),
    }
}

pub fn source_wallhaven_prefer(source: &SourceEntry) -> WallhavenPrefer {
    source.prefer.unwrap_or_else(default_prefer)
}

pub fn main_monitor_resolution_from_cosmic_randr(stdout: &[u8]) -> Option<(u32, u32)> {
    let text = strip_ansi_codes(std::str::from_utf8(stdout).ok()?);

    text.lines()
        .filter(|line| line.contains("(current)"))
        .filter_map(resolution_from_cosmic_randr_line)
        .max_by_key(|&(width, height)| resolution_area(width, height))
}

pub fn main_monitor_resolution_from_xrandr(stdout: &[u8]) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(stdout).ok()?;
    let connected = text.lines().filter(|line| line.contains(" connected "));

    connected
        .clone()
        .find(|line| line.contains(" primary "))
        .and_then(resolution_from_xrandr_line)
        .or_else(|| {
            connected
                .filter_map(resolution_from_xrandr_line)
                .max_by_key(|&(width, height)| resolution_area(width, height))
        })
}

fn main_monitor_resolution() -> Option<(u32, u32)> {
    cosmic_randr_monitor_resolution().or_else(xrandr_monitor_resolution)
}

fn cosmic_randr_monitor_resolution() -> Option<(u32, u32)> {
    let output = Command::new("cosmic-randr").arg("list").output().ok()?;
    if !output.status.success() {
        return None;
    }
    main_monitor_resolution_from_cosmic_randr(&output.stdout)
}

fn xrandr_monitor_resolution() -> Option<(u32, u32)> {
    let output = Command::new("xrandr").arg("--current").output().ok()?;
    if !output.status.success() {
        return None;
    }
    main_monitor_resolution_from_xrandr(&output.stdout)
}

fn resolution_from_xrandr_line(line: &str) -> Option<(u32, u32)> {
    line.split_whitespace()
        .filter_map(|part| part.split_once('+').map(|(resolution, _)| resolution))
        .filter_map(parse_resolution)
        .find(|(width, height)| *width > 0 && *height > 0)
}

fn resolution_from_cosmic_randr_line(line: &str) -> Option<(u32, u32)> {
    line.split_whitespace().find_map(parse_resolution)
}

fn parse_resolution(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn ratio_delta(monitor_ratio: f64, candidate: &str) -> f64 {
    parse_resolution(candidate).map_or(f64::INFINITY, |(width, height)| {
        (monitor_ratio - f64::from(width) / f64::from(height)).abs()
    })
}

fn strip_ansi_codes(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            stripped.push(ch);
        }
    }
    stripped
}

fn resolution_area(width: u32, height: u32) -> u64 {
    u64::from(width) * u64::from(height)
}

#[cfg(test)]
mod tests {
    use super::{
        default_wallhaven_source, main_monitor_resolution_from_cosmic_randr,
        main_monitor_resolution_from_xrandr, source_wallhaven_search,
        wallhaven_atleast_for_monitor, wallhaven_ratio_for_monitor, WallhavenSearch,
    };

    #[test]
    fn default_search_query_is_space() {
        assert_eq!(WallhavenSearch::default().q, "space");
    }

    #[test]
    fn missing_search_query_deserializes_to_space() {
        let search: WallhavenSearch =
            serde_json::from_value(serde_json::json!({})).expect("wallhaven search config");

        assert_eq!(search.q, "space");
    }

    #[test]
    fn blank_source_query_builds_empty_search_query() {
        let source = crate::config::SourceEntry {
            source_type: "wallhaven".into(),
            query: Some("   ".into()),
            ..default_wallhaven_source()
        };

        assert_eq!(source_wallhaven_search(&source).q, "");
    }

    #[test]
    fn monitor_resolution_maps_to_closest_supported_lower_bound() {
        assert_eq!(wallhaven_atleast_for_monitor(2880, 1920), "2560x1440");
        assert_eq!(wallhaven_atleast_for_monitor(3840, 2160), "3840x2160");
        assert_eq!(wallhaven_atleast_for_monitor(1366, 768), "1366x768");
    }

    #[test]
    fn monitor_resolution_maps_to_closest_supported_ratio() {
        assert_eq!(wallhaven_ratio_for_monitor(3360, 2100), "16x10");
        assert_eq!(wallhaven_ratio_for_monitor(2880, 1920), "3x2");
        assert_eq!(wallhaven_ratio_for_monitor(3840, 2160), "16x9");
        assert_eq!(wallhaven_ratio_for_monitor(3440, 1440), "21x9");
        assert_eq!(wallhaven_ratio_for_monitor(1080, 1920), "9x16");
    }

    #[test]
    fn monitor_resolution_falls_back_when_no_choice_fits() {
        assert_eq!(wallhaven_atleast_for_monitor(800, 600), "1920x1080");
    }

    #[test]
    fn xrandr_parser_prefers_primary_monitor_resolution() {
        let stdout = b"HDMI-A-1 connected 3840x2160+1920+0\nDP-1 connected primary 2560x1440+0+0\n";

        assert_eq!(
            main_monitor_resolution_from_xrandr(stdout),
            Some((2560, 1440))
        );
    }

    #[test]
    fn xrandr_parser_falls_back_to_largest_connected_monitor() {
        let stdout = b"HDMI-A-1 connected 1920x1080+0+0\nDP-1 connected 3440x1440+1920+0\nDP-2 disconnected\n";

        assert_eq!(
            main_monitor_resolution_from_xrandr(stdout),
            Some((3440, 1440))
        );
    }

    #[test]
    fn cosmic_randr_parser_uses_current_mode_and_strips_ansi_codes() {
        let stdout = b"\x1b[1meDP-1\x1b[0m \x1b[1;32m(enabled)\x1b[0m\n  Modes:\x1b[0m\n    \x1b[35m2880x1920\x1b[0m @ \x1b[36m120.000 Hz\x1b[0m\x1b[1;35m (current)\x1b[0m\x1b[1;32m (preferred)\x1b[0m\n    \x1b[35m1920x1080\x1b[0m @ \x1b[36m120.000 Hz\x1b[0m\n";

        assert_eq!(
            main_monitor_resolution_from_cosmic_randr(stdout),
            Some((2880, 1920))
        );
    }

    #[test]
    fn cosmic_randr_parser_picks_largest_current_mode_when_multiple_outputs_are_enabled() {
        let stdout = b"eDP-1 (enabled)\n  Modes:\n    1920x1080 @ 60.000 Hz (current)\nHDMI-A-1 (enabled)\n  Modes:\n    3840x2160 @ 60.000 Hz (current)\n";

        assert_eq!(
            main_monitor_resolution_from_cosmic_randr(stdout),
            Some((3840, 2160))
        );
    }
}

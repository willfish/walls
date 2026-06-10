use std::collections::HashMap;

use walls_core::config::{ApplyBackendSetting, Config, CosmicMethod, TuiKeyProfile};

use super::App;

pub(super) fn rotation_draft(config: &Config) -> HashMap<String, String> {
    let mut vals = HashMap::new();
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

pub(super) fn tui_draft(config: &Config) -> HashMap<String, String> {
    let mut vals = HashMap::new();
    vals.insert(
        "key_profile".into(),
        tui_key_profile_label(config.tui.key_profile).into(),
    );
    vals
}

pub(super) fn library_draft(config: &Config) -> HashMap<String, String> {
    let mut vals = HashMap::new();
    vals.insert("quota_enabled".into(), config.quota.enabled.to_string());
    vals.insert("quota_size_mb".into(), config.quota.size_mb.to_string());
    vals.insert(
        "use_landscape_enabled".into(),
        config.selection.use_landscape_enabled.to_string(),
    );
    vals.insert(
        "avoid_recent".into(),
        config.selection.avoid_recent.to_string(),
    );
    vals.insert(
        "refetch_when_cache_below".into(),
        config.selection.refetch_when_cache_below.to_string(),
    );
    vals
}

pub(super) fn display_draft(config: &Config) -> HashMap<String, String> {
    let mut vals = HashMap::new();
    vals.insert(
        "apply_backend".into(),
        apply_backend_label(config.apply.backend).into(),
    );
    vals.insert(
        "custom_script".into(),
        config.apply.custom_script.clone().unwrap_or_default(),
    );
    vals.insert(
        "cosmic_method".into(),
        cosmic_method_label(config.apply.cosmic.method).into(),
    );
    vals.insert(
        "cosmic_config_path".into(),
        config.apply.cosmic.config_path.clone(),
    );
    vals.insert(
        "cosmic_use_original_path".into(),
        config.apply.cosmic.use_original_path.to_string(),
    );
    vals.insert("display_mode".into(), config.display.mode.clone());
    vals.insert("auto_rotate".into(), config.display.auto_rotate.to_string());
    vals.insert(
        "imagemagick_command".into(),
        config.display.imagemagick_command.clone(),
    );
    vals.insert(
        "target_width".into(),
        config
            .display
            .target_width
            .map(|width| width.to_string())
            .unwrap_or_default(),
    );
    vals.insert(
        "target_height".into(),
        config
            .display
            .target_height
            .map(|height| height.to_string())
            .unwrap_or_default(),
    );
    vals.insert(
        "filters_enabled".into(),
        config.display.filters.enabled.to_string(),
    );
    vals.insert(
        "filters_command".into(),
        config.display.filters.command.clone(),
    );
    vals
}

pub(super) fn tui_key_profile_label(profile: TuiKeyProfile) -> &'static str {
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

pub(super) fn apply_backend_label(backend: ApplyBackendSetting) -> &'static str {
    match backend {
        ApplyBackendSetting::Auto => "auto",
        ApplyBackendSetting::Cosmic => "cosmic",
        ApplyBackendSetting::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
        ApplyBackendSetting::Gnome => "gnome",
        ApplyBackendSetting::Kde => "kde",
        ApplyBackendSetting::Xfce => "xfce",
        ApplyBackendSetting::Sway => "sway",
        ApplyBackendSetting::Wlroots => "wlroots",
        ApplyBackendSetting::Hyprland => "hyprland",
        ApplyBackendSetting::Feh => "feh",
        ApplyBackendSetting::CustomScript => "custom-script",
    }
}

fn parse_apply_backend(value: &str) -> Option<ApplyBackendSetting> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(ApplyBackendSetting::Auto),
        "cosmic" => Some(ApplyBackendSetting::Cosmic),
        "cosmic-ext-bg-ctl" => Some(ApplyBackendSetting::CosmicExtBgCtl),
        "gnome" => Some(ApplyBackendSetting::Gnome),
        "kde" => Some(ApplyBackendSetting::Kde),
        "xfce" => Some(ApplyBackendSetting::Xfce),
        "sway" => Some(ApplyBackendSetting::Sway),
        "wlroots" => Some(ApplyBackendSetting::Wlroots),
        "hyprland" => Some(ApplyBackendSetting::Hyprland),
        "feh" => Some(ApplyBackendSetting::Feh),
        "custom-script" => Some(ApplyBackendSetting::CustomScript),
        _ => None,
    }
}

pub(super) fn cosmic_method_label(method: CosmicMethod) -> &'static str {
    match method {
        CosmicMethod::CosmicConfig => "cosmic-config",
        CosmicMethod::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
    }
}

fn parse_cosmic_method(value: &str) -> Option<CosmicMethod> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cosmic-config" => Some(CosmicMethod::CosmicConfig),
        "cosmic-ext-bg-ctl" => Some(CosmicMethod::CosmicExtBgCtl),
        _ => None,
    }
}

pub(super) fn apply_tui_draft(config: &mut Config, draft: &HashMap<String, String>) {
    if let Some(profile) = draft
        .get("key_profile")
        .and_then(|v| parse_tui_key_profile(v))
    {
        config.tui.key_profile = profile;
    }
}

pub(super) fn apply_library_draft(config: &mut Config, draft: &HashMap<String, String>) {
    if let Some(v) = draft.get("quota_enabled") {
        config.quota.enabled = App::parse_bool_like(v).unwrap_or(config.quota.enabled);
    }
    if let Some(v) = draft.get("quota_size_mb") {
        if let Ok(size_mb) = v.parse::<u64>() {
            config.quota.size_mb = size_mb;
        }
    }
    if let Some(v) = draft.get("use_landscape_enabled") {
        config.selection.use_landscape_enabled =
            App::parse_bool_like(v).unwrap_or(config.selection.use_landscape_enabled);
    }
    if let Some(v) = draft.get("avoid_recent") {
        if let Ok(avoid_recent) = v.parse::<usize>() {
            config.selection.avoid_recent = avoid_recent;
        }
    }
    if let Some(v) = draft.get("refetch_when_cache_below") {
        if let Ok(refetch_when_cache_below) = v.parse::<usize>() {
            config.selection.refetch_when_cache_below = refetch_when_cache_below;
        }
    }
}

fn parse_optional_u32(value: &str) -> Option<Option<u32>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(None);
    }
    trimmed.parse::<u32>().ok().filter(|n| *n > 0).map(Some)
}

pub(super) fn apply_display_draft(config: &mut Config, draft: &HashMap<String, String>) {
    if let Some(backend) = draft
        .get("apply_backend")
        .and_then(|v| parse_apply_backend(v))
    {
        config.apply.backend = backend;
    }
    if let Some(v) = draft.get("custom_script") {
        let trimmed = v.trim();
        config.apply.custom_script = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(method) = draft
        .get("cosmic_method")
        .and_then(|v| parse_cosmic_method(v))
    {
        config.apply.cosmic.method = method;
    }
    if let Some(v) = draft.get("cosmic_config_path") {
        config.apply.cosmic.config_path = v.trim().to_string();
    }
    if let Some(v) = draft.get("cosmic_use_original_path") {
        config.apply.cosmic.use_original_path =
            App::parse_bool_like(v).unwrap_or(config.apply.cosmic.use_original_path);
    }
    if let Some(v) = draft.get("display_mode") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            config.display.mode = trimmed.to_string();
        }
    }
    if let Some(v) = draft.get("auto_rotate") {
        config.display.auto_rotate = App::parse_bool_like(v).unwrap_or(config.display.auto_rotate);
    }
    if let Some(v) = draft.get("imagemagick_command") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            config.display.imagemagick_command = trimmed.to_string();
        }
    }
    if let Some(v) = draft
        .get("target_width")
        .and_then(|v| parse_optional_u32(v))
    {
        config.display.target_width = v;
    }
    if let Some(v) = draft
        .get("target_height")
        .and_then(|v| parse_optional_u32(v))
    {
        config.display.target_height = v;
    }
    if let Some(v) = draft.get("filters_enabled") {
        config.display.filters.enabled =
            App::parse_bool_like(v).unwrap_or(config.display.filters.enabled);
    }
    if let Some(v) = draft.get("filters_command") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            config.display.filters.command = trimmed.to_string();
        }
    }
}

pub(super) fn apply_rotation_draft(config: &mut Config, draft: &HashMap<String, String>) {
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

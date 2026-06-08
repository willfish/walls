use crate::config::UnsplashSourceConfig;
use crate::config::{ApplyBackendSetting, Config, Secrets, SourceEntry};
use crate::paths::{expand_home, WallsPaths};

/// Log non-fatal config problems at load time (see also `walls config validate`).
pub fn warn_validation_issues(config: &Config, secrets: &Secrets, paths: &WallsPaths) {
    for issue in validate_config(config, secrets, paths) {
        tracing::warn!(issue, "config validation");
    }
    for issue in secrets_file_permission_warnings(paths) {
        tracing::warn!(issue, "secrets file permissions");
    }
}

/// Full config check for `walls config validate` and non-blocking TUI warnings.
pub fn validate_config(config: &Config, secrets: &Secrets, paths: &WallsPaths) -> Vec<String> {
    let mut errors = Vec::new();

    if !paths.config_file.is_file() {
        errors.push(format!(
            "config file not found: {}",
            paths.config_file.display()
        ));
    }

    for (index, src) in config.sources.iter().enumerate() {
        validate_source_entry(index, src, config, secrets, paths, &mut errors);
    }

    validate_wallhaven_provider(config, secrets, &mut errors);
    validate_apply_config(config, &mut errors);
    validate_quota_config(config, &mut errors);
    validate_tray_autostart(config, &mut errors);

    errors
}

/// Validate one source entry while editing it in the TUI (no global config checks).
pub fn validate_source_edit(
    index: usize,
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(src) = config.sources.get(index) else {
        errors.push(format!("source #{index} does not exist"));
        return errors;
    };
    validate_source_entry(index, src, config, secrets, paths, &mut errors);
    errors
}

/// Validate the Wallhaven provider block while editing it in the TUI.
pub fn validate_wallhaven_edit(config: &Config, secrets: &Secrets) -> Vec<String> {
    let mut errors = Vec::new();
    validate_wallhaven_provider(config, secrets, &mut errors);
    errors
}

fn validate_source_entry(
    index: usize,
    src: &SourceEntry,
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
    errors: &mut Vec<String>,
) {
    let _ = index;
    if src.source_type.trim().is_empty() {
        errors.push(format!(
            "source {:?}: type is required",
            src.label.as_deref().unwrap_or("(unnamed)")
        ));
        return;
    }

    if !src.enabled {
        return;
    }

    match src.source_type.as_str() {
        "folder" | "image" | "favorites" | "fetched" => {
            let expanded = match src.source_type.as_str() {
                "favorites" => paths.favorites_dir.clone(),
                "fetched" => paths.fetched_dir.clone(),
                _ => {
                    let Some(path) = src.path.as_ref() else {
                        errors.push(format!(
                            "source {:?}: missing path for type {}",
                            src.label, src.source_type
                        ));
                        return;
                    };
                    expand_home(path)
                }
            };
            if !expanded.exists() {
                errors.push(format!(
                    "source {:?}: path does not exist: {}",
                    src.label,
                    expanded.display()
                ));
            }
        }
        "unsplash" => {
            if config.change.internet_enabled && secrets.unsplash_access_key.is_empty() {
                errors.push(
                    "unsplash source enabled but secrets.unsplash_access_key is empty".into(),
                );
            }
            if let Err(error) = UnsplashSourceConfig::from_source(src) {
                errors.push(format!("source {:?}: {error}", src.label));
            }
        }
        _ => {}
    }
}

fn validate_wallhaven_provider(_config: &Config, _secrets: &Secrets, _errors: &mut Vec<String>) {}

fn validate_tray_autostart(config: &Config, errors: &mut Vec<String>) {
    let Ok(config_home) = autostart_config_home() else {
        return;
    };
    let tray_bin = crate::bin_resolve::resolve_binary(crate::bin_resolve::BinResolveOpts {
        env_var: std::env::var("WALLS_TRAY_BIN").ok().as_deref(),
        current_exe: std::env::current_exe().ok().as_deref(),
        sibling_name: "walls-tray",
        build_default: None,
        path_fallback: "walls-tray",
    });
    let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let xdg_session_desktop = std::env::var("XDG_SESSION_DESKTOP").ok();
    let desktop_startup_id = std::env::var("DESKTOP_STARTUP_ID").ok();
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();
    let opts = crate::autostart::AutostartSyncOpts {
        config_home: &config_home,
        tray_bin,
        config,
        xdg_current_desktop: xdg_current_desktop.as_deref(),
        xdg_session_desktop: xdg_session_desktop.as_deref(),
        desktop_startup_id: desktop_startup_id.as_deref(),
        xdg_session_type: xdg_session_type.as_deref(),
        wayland_display: wayland_display.as_deref(),
        display: display.as_deref(),
    };
    if crate::autostart::autostart_out_of_sync(&opts) {
        errors.push(
            "tray autostart desktop entry is out of sync with config; run `walls config sync`"
                .into(),
        );
    }
}

fn autostart_config_home() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Ok(std::path::PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME")?;
    Ok(std::path::PathBuf::from(home).join(".config"))
}

fn validate_apply_config(config: &Config, errors: &mut Vec<String>) {
    let custom_script = config
        .apply
        .custom_script
        .as_deref()
        .filter(|path| !path.trim().is_empty());

    match (config.apply.backend, custom_script) {
        (ApplyBackendSetting::CustomScript, Some(script)) => {
            let script_path = expand_home(script);
            if !script_path.is_file() {
                errors.push(format!(
                    "apply.custom_script not found or not a file: {}",
                    script_path.display()
                ));
                return;
            }
            #[cfg(unix)]
            if !is_executable(&script_path) {
                errors.push(format!(
                    "apply.custom_script is not executable: {}; run `chmod +x {}`",
                    script_path.display(),
                    script_path.display()
                ));
            }
        }
        (ApplyBackendSetting::CustomScript, None) => {
            errors
                .push("apply.custom_script is required when apply.backend is custom-script".into());
        }
        (backend, Some(_)) => {
            errors.push(format!(
                "apply.custom_script is set but apply.backend is {}; set apply.backend to custom-script or remove apply.custom_script",
                apply_backend_name(backend)
            ));
        }
        (_, None) => {}
    }

    if config.apply.backend == ApplyBackendSetting::Cosmic {
        let cosmic_path = expand_home(&config.apply.cosmic.config_path);
        if !cosmic_path.is_file() {
            errors.push(format!(
                "apply.cosmic.config_path not found: {}",
                cosmic_path.display()
            ));
        }
    }
}

fn validate_quota_config(config: &Config, errors: &mut Vec<String>) {
    if config.quota.size_mb == 0 {
        errors.push("quota.size_mb must be greater than zero".into());
    }
}

fn apply_backend_name(backend: ApplyBackendSetting) -> &'static str {
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

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[allow(
        clippy::verbose_bit_mask,
        reason = "custom script validation intentionally accepts any owner/group/other execute bit."
    )]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
}

#[cfg(unix)]
pub fn secrets_file_permission_warnings(paths: &WallsPaths) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(&paths.secrets_file) else {
        return Vec::new();
    };
    let mode = metadata.permissions().mode();
    #[allow(
        clippy::verbose_bit_mask,
        reason = "the secrets permission warning intentionally checks all group/other permission bits in one mode mask."
    )]
    if mode & 0o077 == 0 {
        return Vec::new();
    }

    vec![format!(
        "secrets file is readable by group or other users: {}; run `chmod 600 {}`",
        paths.secrets_file.display(),
        paths.secrets_file.display()
    )]
}

#[cfg(not(unix))]
pub fn secrets_file_permission_warnings(_paths: &WallsPaths) -> Vec<String> {
    Vec::new()
}

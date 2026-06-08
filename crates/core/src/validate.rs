use crate::config::UnsplashSourceConfig;
use crate::config::{ApplyBackendSetting, Config, Secrets};
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

pub fn validate_config(config: &Config, secrets: &Secrets, paths: &WallsPaths) -> Vec<String> {
    let mut errors = Vec::new();

    if !paths.config_file.is_file() {
        errors.push(format!(
            "config file not found: {}",
            paths.config_file.display()
        ));
    }

    for src in &config.sources {
        if !src.enabled {
            continue;
        }
        match src.source_type.as_str() {
            "folder" | "image" => {
                let Some(path) = src.path.as_ref() else {
                    errors.push(format!(
                        "source {:?}: missing path for type {}",
                        src.label, src.source_type
                    ));
                    continue;
                };
                let expanded = expand_home(path);
                if !expanded.exists() {
                    errors.push(format!(
                        "source {:?}: path does not exist: {}",
                        src.label,
                        expanded.display()
                    ));
                }
            }
            "wallhaven"
                if config.change.internet_enabled && secrets.wallhaven_api_key.is_empty() =>
            {
                errors
                    .push("wallhaven source enabled but secrets.wallhaven_api_key is empty".into());
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

    if config.wallhaven.enabled
        && config.change.internet_enabled
        && secrets.wallhaven_api_key.is_empty()
    {
        errors.push("wallhaven provider enabled but secrets.wallhaven_api_key is empty".into());
    }

    validate_apply_config(config, &mut errors);
    validate_quota_config(config, &mut errors);

    errors
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

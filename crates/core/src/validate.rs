use crate::config::{ApplyBackendSetting, Config, Secrets};
use crate::paths::{expand_home, WallsPaths};

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
            _ => {}
        }
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

    errors
}

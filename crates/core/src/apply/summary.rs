use crate::config::{ApplyBackendSetting, ApplyConfig, CosmicMethod};
use crate::paths::expand_home;

use super::{auto_backend_for_desktop, detect_desktop_from_env, Desktop};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyEnvironmentSummary {
    pub xdg_current_desktop: Option<String>,
    pub xdg_session_desktop: Option<String>,
    pub xdg_session_type: Option<String>,
    pub detected_desktop: Desktop,
    pub configured_backend: ApplyBackendSetting,
    pub resolved_backend: ApplyBackendSetting,
    pub uses_feh_fallback: bool,
    pub cosmic_config_path: Option<String>,
    pub cosmic_config_exists: Option<bool>,
}

impl ApplyEnvironmentSummary {
    pub fn for_config(apply: &ApplyConfig) -> Self {
        Self::from_env(
            apply,
            std::env::var("XDG_CURRENT_DESKTOP").ok(),
            std::env::var("XDG_SESSION_DESKTOP").ok(),
            std::env::var("XDG_SESSION_TYPE").ok(),
        )
    }

    pub fn from_env(
        apply: &ApplyConfig,
        xdg_current_desktop: Option<String>,
        xdg_session_desktop: Option<String>,
        xdg_session_type: Option<String>,
    ) -> Self {
        let detected_desktop = detect_desktop_from_env(
            xdg_current_desktop.as_deref(),
            xdg_session_desktop.as_deref(),
            None,
        );
        let configured_backend = apply.backend;
        let resolved_backend = if configured_backend == ApplyBackendSetting::Auto {
            auto_backend_for_desktop(detected_desktop)
        } else {
            configured_backend
        };
        let uses_feh_fallback = resolved_backend == ApplyBackendSetting::Auto;
        let cosmic_config_path = (resolved_backend == ApplyBackendSetting::Cosmic
            || resolved_backend == ApplyBackendSetting::CosmicExtBgCtl
            || configured_backend == ApplyBackendSetting::Cosmic
            || configured_backend == ApplyBackendSetting::CosmicExtBgCtl)
            .then(|| apply.cosmic.config_path.clone());
        let cosmic_config_exists = cosmic_config_path.as_ref().map(|path| {
            let expanded = expand_home(path);
            expanded.is_file() || expanded.exists()
        });

        Self {
            xdg_current_desktop,
            xdg_session_desktop,
            xdg_session_type,
            detected_desktop,
            configured_backend,
            resolved_backend,
            uses_feh_fallback,
            cosmic_config_path,
            cosmic_config_exists,
        }
    }

    pub fn effective_backend_label(&self) -> &'static str {
        if self.uses_feh_fallback {
            "feh/nitrogen (fallback)"
        } else {
            backend_setting_label(self.resolved_backend)
        }
    }

    pub fn cosmic_method_label(&self, method: CosmicMethod) -> &'static str {
        match method {
            CosmicMethod::CosmicConfig => "cosmic-config (RON patch)",
            CosmicMethod::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
        }
    }
}

pub fn summarize_apply_environment(apply: &ApplyConfig) -> ApplyEnvironmentSummary {
    ApplyEnvironmentSummary::for_config(apply)
}

pub fn summarize_apply_environment_from_env(
    apply: &ApplyConfig,
    xdg_current_desktop: Option<String>,
    xdg_session_desktop: Option<String>,
    xdg_session_type: Option<String>,
) -> ApplyEnvironmentSummary {
    ApplyEnvironmentSummary::from_env(
        apply,
        xdg_current_desktop,
        xdg_session_desktop,
        xdg_session_type,
    )
}

pub fn desktop_display_name(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Gnome => "GNOME",
        Desktop::Unity => "Unity",
        Desktop::Budgie => "Budgie",
        Desktop::Kde => "KDE Plasma",
        Desktop::Xfce => "Xfce",
        Desktop::Lxde => "LXDE",
        Desktop::Lxqt => "LXQt",
        Desktop::Mate => "MATE",
        Desktop::Cinnamon => "Cinnamon",
        Desktop::Lingmo => "Lingmo",
        Desktop::Deepin => "Deepin",
        Desktop::Trinity => "Trinity",
        Desktop::Fluxbox => "Fluxbox",
        Desktop::Sway => "Sway",
        Desktop::Hyprland => "Hyprland",
        Desktop::Enlightenment => "Enlightenment",
        Desktop::Awesome => "Awesome",
        Desktop::Cosmic => "COSMIC",
        Desktop::Unknown => "unknown",
    }
}

pub fn backend_setting_label(backend: ApplyBackendSetting) -> &'static str {
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

fn env_display(value: Option<&str>) -> String {
    value
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("(unset)")
        .to_string()
}

impl ApplyEnvironmentSummary {
    pub fn detection_detail_lines(&self, cosmic_method: CosmicMethod) -> Vec<String> {
        let mut lines = vec![
            format!(
                "desktop: {} (from XDG_CURRENT_DESKTOP={})",
                desktop_display_name(self.detected_desktop),
                env_display(self.xdg_current_desktop.as_deref())
            ),
            format!(
                "session desktop: {}",
                env_display(self.xdg_session_desktop.as_deref())
            ),
            format!(
                "session type: {}",
                env_display(self.xdg_session_type.as_deref())
            ),
            format!("resolved backend: {}", self.effective_backend_label()),
        ];
        if self.resolved_backend == ApplyBackendSetting::Cosmic
            || self.configured_backend == ApplyBackendSetting::Cosmic
        {
            lines.push(format!(
                "cosmic method: {}",
                self.cosmic_method_label(cosmic_method)
            ));
        }
        if let Some(path) = &self.cosmic_config_path {
            let status = match self.cosmic_config_exists {
                Some(true) => "found",
                Some(false) => "missing",
                None => "unknown",
            };
            lines.push(format!("cosmic config on disk: {status} ({path})"));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApplyConfig;

    #[test]
    fn cosmic_session_resolves_to_cosmic_backend() {
        let summary = ApplyEnvironmentSummary::from_env(
            &ApplyConfig::default(),
            Some("COSMIC".into()),
            None,
            Some("wayland".into()),
        );
        assert_eq!(summary.detected_desktop, Desktop::Cosmic);
        assert_eq!(summary.resolved_backend, ApplyBackendSetting::Cosmic);
        assert!(!summary.uses_feh_fallback);
        assert_eq!(summary.effective_backend_label(), "cosmic");
    }

    #[test]
    fn unknown_desktop_uses_feh_fallback_when_backend_auto() {
        let summary = ApplyEnvironmentSummary::from_env(
            &ApplyConfig::default(),
            None,
            None,
            Some("x11".into()),
        );
        assert_eq!(summary.detected_desktop, Desktop::Unknown);
        assert!(summary.uses_feh_fallback);
        assert_eq!(summary.effective_backend_label(), "feh/nitrogen (fallback)");
    }

    #[test]
    fn explicit_backend_skips_auto_resolution() {
        let apply = ApplyConfig {
            backend: ApplyBackendSetting::CustomScript,
            ..ApplyConfig::default()
        };
        let summary = ApplyEnvironmentSummary::from_env(&apply, Some("COSMIC".into()), None, None);
        assert_eq!(summary.resolved_backend, ApplyBackendSetting::CustomScript);
        assert!(!summary.uses_feh_fallback);
    }

    #[test]
    fn detection_lines_include_session_env() {
        let summary = ApplyEnvironmentSummary::from_env(
            &ApplyConfig::default(),
            Some("GNOME".into()),
            Some("ubuntu".into()),
            Some("wayland".into()),
        );
        let lines = summary.detection_detail_lines(CosmicMethod::CosmicConfig);
        assert!(lines.iter().any(|l| l.contains("GNOME")));
        assert!(lines.iter().any(|l| l.contains("session type: wayland")));
        assert!(lines.iter().any(|l| l.contains("resolved backend: gnome")));
    }
}

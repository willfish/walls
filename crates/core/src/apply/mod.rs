mod cosmic;
mod detect;
mod feh_nitrogen;
mod file_uri;
pub mod fill_mode;
mod gnome;
mod kde;
mod summary;
mod wlroots;
mod xfce;

pub use cosmic::{patch_cosmic_background, patch_wallpaper_path, CosmicConfigApplier};
pub use detect::{detect_desktop, detect_desktop_from_env, Desktop};
pub use feh_nitrogen::FehNitrogenApplier;
pub use fill_mode::{ApplyTrigger, FillMode};
pub use gnome::{gnome_gsettings_commands, GnomeApplier};
pub use kde::{kde_dbus_send_args, plasma_script, unsupported_plugins_from_dbus_reply, KdeApplier};
pub use summary::{
    backend_setting_label, desktop_display_name, summarize_apply_environment,
    summarize_apply_environment_from_env, ApplyEnvironmentSummary,
};
pub use wlroots::{
    hyprctl_monitors_args, hyprland_monitor_names, sway_output_bg_args, wlroots_scale_mode,
    wlroots_swaybg_commands, HyprlandApplier, SwayApplier, WlrootsApplier,
};
pub use xfce::{
    connected_xrandr_monitors, xfce_existing_backdrop_properties, xfce_existing_property_commands,
    xfce_list_backdrop_args, xfce_new_monitor_commands, XfceApplier,
};

use std::path::Path;
use std::process::Command;

use crate::config::{ApplyBackendSetting, ApplyConfig, CosmicMethod};
use crate::paths::expand_home;

pub trait Applier: Send + Sync {
    fn set_wallpaper(
        &self,
        display: &Path,
        original: &Path,
        fill: FillMode,
        trigger: ApplyTrigger,
    ) -> anyhow::Result<()>;
}

pub struct CustomScriptApplier {
    script: std::path::PathBuf,
}

impl CustomScriptApplier {
    pub fn new(script: impl AsRef<Path>) -> Self {
        Self {
            script: expand_home(script),
        }
    }
}

impl Applier for CustomScriptApplier {
    fn set_wallpaper(
        &self,
        display: &Path,
        original: &Path,
        fill: FillMode,
        trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        let fill_str = fill.gnome_picture_options().unwrap_or("os");
        let status = Command::new(&self.script)
            .arg(display)
            .arg(trigger.as_str())
            .arg(original)
            .arg(fill_str)
            .status()?;
        if !status.success() {
            anyhow::bail!("custom apply script failed: {status}");
        }
        Ok(())
    }
}

pub fn build_applier(apply: &ApplyConfig) -> anyhow::Result<Box<dyn Applier>> {
    match resolve_backend(apply) {
        ApplyBackendSetting::CustomScript => {
            let script = apply
                .custom_script
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("apply.custom_script is not set"))?;
            Ok(Box::new(CustomScriptApplier::new(script)))
        }
        ApplyBackendSetting::Gnome => Ok(Box::new(GnomeApplier)),
        ApplyBackendSetting::Kde => Ok(Box::new(KdeApplier)),
        ApplyBackendSetting::Xfce => Ok(Box::new(XfceApplier)),
        ApplyBackendSetting::Sway => Ok(Box::new(SwayApplier)),
        ApplyBackendSetting::Wlroots => Ok(Box::new(WlrootsApplier)),
        ApplyBackendSetting::Hyprland => Ok(Box::new(HyprlandApplier)),
        ApplyBackendSetting::Feh => Ok(Box::new(FehNitrogenApplier)),
        ApplyBackendSetting::CosmicExtBgCtl => Ok(cosmic::build_cosmic_applier(&ApplyConfig {
            cosmic: crate::config::CosmicApplyConfig {
                method: CosmicMethod::CosmicExtBgCtl,
                ..apply.cosmic.clone()
            },
            ..apply.clone()
        })),
        ApplyBackendSetting::Cosmic => Ok(cosmic::build_cosmic_applier(apply)),
        ApplyBackendSetting::Auto => {
            tracing::warn!(desktop = ?detect_desktop(), "falling back to feh/nitrogen");
            Ok(Box::new(FehNitrogenApplier))
        }
    }
}

/// Backend that will actually run for this config in the current session.
pub fn resolved_apply_backend(apply: &ApplyConfig) -> ApplyBackendSetting {
    match apply.backend {
        ApplyBackendSetting::Auto => auto_backend_for_desktop(detect_desktop()),
        other => other,
    }
}

fn resolve_backend(apply: &ApplyConfig) -> ApplyBackendSetting {
    resolved_apply_backend(apply)
}

pub fn auto_backend_for_desktop(desktop: Desktop) -> ApplyBackendSetting {
    match desktop {
        Desktop::Cosmic => ApplyBackendSetting::Cosmic,
        Desktop::Gnome | Desktop::Unity | Desktop::Budgie => ApplyBackendSetting::Gnome,
        Desktop::Kde => ApplyBackendSetting::Kde,
        Desktop::Xfce => ApplyBackendSetting::Xfce,
        Desktop::Sway => ApplyBackendSetting::Sway,
        Desktop::Hyprland => ApplyBackendSetting::Hyprland,
        _ => ApplyBackendSetting::Auto,
    }
}

pub fn apply_wallpaper(
    apply: &ApplyConfig,
    composed: &Path,
    original: &Path,
    fill: FillMode,
    trigger: ApplyTrigger,
) -> anyhow::Result<()> {
    let display = if apply.cosmic.use_original_path {
        original
    } else {
        composed
    };
    let applier = build_applier(apply)?;
    applier.set_wallpaper(display, original, fill, trigger)
}

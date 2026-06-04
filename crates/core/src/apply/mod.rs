mod cosmic;
mod detect;
mod feh_nitrogen;
pub mod fill_mode;
mod gnome;

pub use cosmic::{patch_wallpaper_path, CosmicConfigApplier};
pub use detect::{detect_desktop, detect_desktop_from_env, Desktop};
pub use feh_nitrogen::FehNitrogenApplier;
pub use fill_mode::{ApplyTrigger, FillMode};
pub use gnome::{gnome_gsettings_commands, GnomeApplier};

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
        ApplyBackendSetting::Feh => Ok(Box::new(FehNitrogenApplier)),
        ApplyBackendSetting::CosmicExtBgCtl => cosmic::build_cosmic_applier(&ApplyConfig {
            cosmic: crate::config::CosmicApplyConfig {
                method: CosmicMethod::CosmicExtBgCtl,
                ..apply.cosmic.clone()
            },
            ..apply.clone()
        }),
        ApplyBackendSetting::Cosmic => cosmic::build_cosmic_applier(apply),
        ApplyBackendSetting::Auto => {
            tracing::warn!(desktop = ?detect_desktop(), "falling back to feh/nitrogen");
            Ok(Box::new(FehNitrogenApplier))
        }
    }
}

fn resolve_backend(apply: &ApplyConfig) -> ApplyBackendSetting {
    match apply.backend {
        ApplyBackendSetting::Auto => match detect_desktop() {
            Desktop::Cosmic => ApplyBackendSetting::Cosmic,
            Desktop::Gnome | Desktop::Unity | Desktop::Budgie => ApplyBackendSetting::Gnome,
            _ => ApplyBackendSetting::Auto,
        },
        other => other,
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

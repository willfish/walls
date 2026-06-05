use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;

use crate::config::{ApplyConfig, CosmicMethod};
use crate::paths::expand_home;

use super::fill_mode::FillMode;
use super::Applier;

/// Patch `source: Path("...")` in COSMIC background RON config (Variety-compatible).
pub fn patch_wallpaper_path(contents: &str, new_path: &Path) -> String {
    let escaped = new_path
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let replacement = format!(r#"source: Path("{escaped}")"#);
    Regex::new(r#"source: Path\("[^"]+"\)"#)
        .expect("valid regex")
        .replace_all(contents, replacement.as_str())
        .into_owned()
}

pub struct CosmicConfigApplier {
    config_path: std::path::PathBuf,
}

impl CosmicConfigApplier {
    pub fn new(config_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: expand_home(config_path),
        }
    }

    pub fn apply_path(&self, wallpaper: &Path) -> anyhow::Result<()> {
        let contents = fs::read_to_string(&self.config_path).map_err(|e| {
            anyhow::anyhow!(
                "failed to read COSMIC background config {}: {e}",
                self.config_path.display()
            )
        })?;
        let patched = patch_wallpaper_path(&contents, wallpaper);
        if patched == contents {
            anyhow::bail!(
                "no source: Path(...) entry found in {}",
                self.config_path.display()
            );
        }
        fs::write(&self.config_path, patched)?;
        tracing::info!(path = %self.config_path.display(), "patched COSMIC wallpaper path");
        Ok(())
    }
}

pub struct CosmicExtBgApplier;

impl Applier for CosmicExtBgApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: super::fill_mode::ApplyTrigger,
    ) -> anyhow::Result<()> {
        let status = Command::new("cosmic-ext-bg-ctl")
            .arg("set")
            .arg(composed)
            .status()?;
        if !status.success() {
            anyhow::bail!("cosmic-ext-bg-ctl set failed with {status}");
        }
        Ok(())
    }
}

impl Applier for CosmicConfigApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: super::fill_mode::ApplyTrigger,
    ) -> anyhow::Result<()> {
        self.apply_path(composed)
    }
}

pub fn build_cosmic_applier(apply: &ApplyConfig) -> Box<dyn Applier> {
    match apply.cosmic.method {
        CosmicMethod::CosmicConfig => Box::new(CosmicConfigApplier::new(&apply.cosmic.config_path)),
        CosmicMethod::CosmicExtBgCtl => Box::new(CosmicExtBgApplier),
    }
}

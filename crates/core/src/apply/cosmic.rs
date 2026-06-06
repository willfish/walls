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
    let source_re = Regex::new(r#"source: Path\("[^"]+"\)"#).expect("valid regex");
    let patched = source_re
        .replace_all(contents, replacement.as_str())
        .into_owned();
    if patched != contents {
        return patched;
    }

    // No existing source entry found.
    // Prefer injecting inside a per-output tuple (realistic COSMIC structure from the native switcher / multi-monitor).
    // This ensures the source is in the idiomatic place the DE uses for that output.
    let output_re = Regex::new(r#"(output:\s*"[^"]+"\s*,\s*)"#).expect("valid regex for output");
    if let Some(caps) = output_re.captures(contents) {
        let prefix = &caps[1];
        let inserted = format!("{prefix}source: Path(\"{escaped}\"), ");
        let m = caps.get(0).unwrap();
        let mut res = contents.to_string();
        res.replace_range(m.start()..m.end(), &inserted);
        if res != contents {
            return res;
        }
    }

    // Fallback for flat/simple backgrounds: ( color: ... ) style (no outputs).
    let insert_re = Regex::new(r"(backgrounds:\s*\(\s*)").expect("valid regex");
    if let Some(caps) = insert_re.captures(contents) {
        let prefix = &caps[1];
        let inserted = format!("{prefix}source: Path(\"{escaped}\"), ");
        let m = caps.get(0).unwrap();
        let mut res = contents.to_string();
        res.replace_range(m.start()..m.end(), &inserted);
        return res;
    }

    // Last resort minimal (may lose other settings)
    format!(r#"backgrounds: ( source: Path("{escaped}"), color: [0.0, 0.0, 0.0, 1.0], )"#)
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
        fs::write(&self.config_path, patched)?;
        tracing::info!(path = %self.config_path.display(), "patched COSMIC wallpaper path");

        // Best-effort force live update via ext ctl (if installed).
        // This makes the wallpaper *actually change* on screen immediately, even if the ron patch
        // alone doesn't trigger the bg daemon's hot-reload/watch (explains "log says applies but no
        // visual change" until native switcher normalized the config file, after which patches took effect).
        // We still wrote the ron for persistence (reboot/login etc.).
        let ctl_res = Command::new("cosmic-ext-bg-ctl")
            .arg("set")
            .arg(wallpaper)
            .status();
        if let Ok(status) = ctl_res {
            if status.success() {
                tracing::info!("also forced live bg via cosmic-ext-bg-ctl set");
            }
        } // else: binary not in PATH or failed; swallow (best effort, ron patch is the main thing)
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

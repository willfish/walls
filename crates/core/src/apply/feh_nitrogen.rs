use std::path::Path;
use std::process::Command;

use super::fill_mode::{ApplyTrigger, FillMode};
use super::Applier;

pub struct FehNitrogenApplier;

impl Applier for FehNitrogenApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        if let Ok(status) = Command::new("feh").arg("--bg-fill").arg(composed).status() {
            if status.success() {
                return Ok(());
            }
        }
        let status = Command::new("nitrogen")
            .args(["--set-zoom-fill", "--save"])
            .arg(composed)
            .status()?;
        if !status.success() {
            anyhow::bail!("neither feh nor nitrogen succeeded setting wallpaper");
        }
        Ok(())
    }
}
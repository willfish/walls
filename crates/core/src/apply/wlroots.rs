use std::path::Path;
use std::process::{Command, Stdio};

use super::fill_mode::{ApplyTrigger, FillMode};
use super::Applier;

pub struct SwayApplier;
pub struct WlrootsApplier;
pub struct HyprlandApplier;

impl Applier for SwayApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        let status = Command::new("swaymsg")
            .args(sway_output_bg_args(composed, fill))
            .status()?;

        if !status.success() {
            anyhow::bail!("swaymsg failed setting sway wallpaper");
        }

        Ok(())
    }
}

impl Applier for WlrootsApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        restart_swaybg(wlroots_swaybg_commands(&[], composed, fill))
    }
}

impl Applier for HyprlandApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        let output = Command::new("hyprctl")
            .args(hyprctl_monitors_args())
            .output()?;

        if !output.status.success() {
            anyhow::bail!("hyprctl failed listing Hyprland monitors");
        }

        restart_swaybg(wlroots_swaybg_commands(
            &hyprland_monitor_names(&output.stdout),
            composed,
            fill,
        ))
    }
}

pub fn sway_output_bg_args(path: &Path, fill: FillMode) -> Vec<String> {
    vec![
        "output".into(),
        "*".into(),
        "bg".into(),
        path.display().to_string(),
        wlroots_scale_mode(fill).into(),
    ]
}

pub fn hyprctl_monitors_args() -> Vec<String> {
    vec!["monitors".into()]
}

pub fn hyprland_monitor_names(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("Monitor ")?;
            let name = rest.split_whitespace().next()?;
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect()
}

pub fn wlroots_swaybg_commands(
    monitors: &[String],
    path: &Path,
    fill: FillMode,
) -> Vec<Vec<String>> {
    if monitors.is_empty() {
        return vec![swaybg_args(None, path, fill)];
    }

    monitors
        .iter()
        .map(|monitor| swaybg_args(Some(monitor), path, fill))
        .collect()
}

pub fn wlroots_scale_mode(fill: FillMode) -> &'static str {
    match fill {
        FillMode::Centered => "center",
        FillMode::Scaled => "fit",
        FillMode::Stretched => "stretch",
        FillMode::Wallpaper => "tile",
        FillMode::Os | FillMode::Zoom | FillMode::Spanned => "fill",
    }
}

fn swaybg_args(output: Option<&str>, path: &Path, fill: FillMode) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(output) = output {
        args.extend(["-o".into(), output.into()]);
    }
    args.extend([
        "-i".into(),
        path.display().to_string(),
        "-m".into(),
        wlroots_scale_mode(fill).into(),
    ]);
    args
}

fn restart_swaybg(commands: Vec<Vec<String>>) -> anyhow::Result<()> {
    let _ = Command::new("pkill").args(["-x", "swaybg"]).status();

    for args in commands {
        Command::new("swaybg")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }

    Ok(())
}

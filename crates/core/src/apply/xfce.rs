use std::path::Path;
use std::process::Command;

use super::fill_mode::{ApplyTrigger, FillMode};
use super::Applier;

const XFCE_CHANNEL: &str = "xfce4-desktop";
const BACKDROP_ROOT: &str = "/backdrop";
const DEFAULT_LAST_IMAGE: &str = "/backdrop/screen0/monitor0/workspace0/last-image";

pub struct XfceApplier;

impl Applier for XfceApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        let list_output = Command::new("xfconf-query")
            .args(xfce_list_backdrop_args())
            .output()?;

        if !list_output.status.success() {
            anyhow::bail!("xfconf-query failed listing XFCE backdrop properties");
        }

        let existing_props = xfce_existing_backdrop_properties(&list_output.stdout);
        if !existing_props.is_empty() {
            run_xfconf_commands(xfce_existing_property_commands(&existing_props, composed))?;
            return Ok(());
        }

        let monitors = Command::new("xrandr")
            .arg("--query")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| connected_xrandr_monitors(&output.stdout))
            .unwrap_or_default();

        run_xfconf_commands(xfce_new_monitor_commands(&monitors, composed))
    }
}

pub fn xfce_list_backdrop_args() -> Vec<String> {
    vec![
        "-c".into(),
        XFCE_CHANNEL.into(),
        "-p".into(),
        BACKDROP_ROOT.into(),
        "-l".into(),
    ]
}

pub fn xfce_existing_backdrop_properties(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with(BACKDROP_ROOT)
                && line.contains("screen")
                && line.contains("/monitor")
                && (line.ends_with("image-path") || line.ends_with("last-image"))
        })
        .map(str::to_owned)
        .collect()
}

pub fn connected_xrandr_monitors(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let state = parts.next()?;
            (state == "connected").then(|| name.to_owned())
        })
        .collect()
}

pub fn xfce_existing_property_commands(properties: &[String], path: &Path) -> Vec<Vec<String>> {
    properties
        .iter()
        .flat_map(|property| {
            [
                xfce_set_string_arg(property, ""),
                xfce_set_arg(property, ""),
                xfce_set_arg(property, &path.display().to_string()),
            ]
        })
        .collect()
}

pub fn xfce_new_monitor_commands(monitors: &[String], path: &Path) -> Vec<Vec<String>> {
    if monitors.is_empty() {
        return vec![xfce_set_string_arg(
            DEFAULT_LAST_IMAGE,
            &path.display().to_string(),
        )];
    }

    monitors
        .iter()
        .flat_map(|monitor| {
            let image_prop = format!("/backdrop/screen0/monitor{monitor}/workspace0/last-image");
            let style_prop = format!("/backdrop/screen0/monitor{monitor}/workspace0/image-style");
            [
                xfce_set_string_arg(&image_prop, &path.display().to_string()),
                xfce_set_int_arg(&style_prop, "5"),
            ]
        })
        .collect()
}

fn run_xfconf_commands(commands: Vec<Vec<String>>) -> anyhow::Result<()> {
    let mut set_image_succeeded = false;
    for args in commands {
        let sets_image = xfce_args_set_wallpaper_path(&args);
        let status = Command::new("xfconf-query").args(&args).status()?;
        if status.success() && sets_image {
            set_image_succeeded = true;
        }
    }

    if !set_image_succeeded {
        anyhow::bail!("xfconf-query failed setting XFCE wallpaper");
    }

    Ok(())
}

fn xfce_args_set_wallpaper_path(args: &[String]) -> bool {
    let property = args
        .windows(2)
        .find_map(|window| (window[0] == "-p").then_some(window[1].as_str()));
    let value = args
        .windows(2)
        .find_map(|window| (window[0] == "-s").then_some(window[1].as_str()));

    property.is_some_and(|property| {
        property.ends_with("image-path") || property.ends_with("last-image")
    }) && value.is_some_and(|value| !value.is_empty())
}

fn xfce_set_string_arg(property: &str, value: &str) -> Vec<String> {
    vec![
        "-c".into(),
        XFCE_CHANNEL.into(),
        "-p".into(),
        property.into(),
        "-n".into(),
        "-t".into(),
        "string".into(),
        "-s".into(),
        value.into(),
    ]
}

fn xfce_set_int_arg(property: &str, value: &str) -> Vec<String> {
    vec![
        "-c".into(),
        XFCE_CHANNEL.into(),
        "-p".into(),
        property.into(),
        "-n".into(),
        "-t".into(),
        "int".into(),
        "-s".into(),
        value.into(),
    ]
}

fn xfce_set_arg(property: &str, value: &str) -> Vec<String> {
    vec![
        "-c".into(),
        XFCE_CHANNEL.into(),
        "-p".into(),
        property.into(),
        "-s".into(),
        value.into(),
    ]
}

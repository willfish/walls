use std::path::Path;

use walls_core::apply::{
    detect_desktop_from_env, hyprctl_monitors_args, hyprland_monitor_names, sway_output_bg_args,
    wlroots_scale_mode, wlroots_swaybg_commands, Desktop, FillMode,
};
use walls_core::config::{ApplyBackendSetting, ApplyConfig};

#[test]
fn detects_sway_and_hyprland_from_xdg_current_desktop() {
    assert_eq!(
        detect_desktop_from_env(Some("sway"), None, None),
        Desktop::Sway
    );
    assert_eq!(
        detect_desktop_from_env(Some("Hyprland"), None, None),
        Desktop::Hyprland
    );
}

#[test]
fn parses_explicit_wlroots_backend_configs() {
    let sway: ApplyConfig = serde_json::from_str(r#"{"backend":"sway"}"#).unwrap();
    let wlroots: ApplyConfig = serde_json::from_str(r#"{"backend":"wlroots"}"#).unwrap();
    let hyprland: ApplyConfig = serde_json::from_str(r#"{"backend":"hyprland"}"#).unwrap();

    assert_eq!(sway.backend, ApplyBackendSetting::Sway);
    assert_eq!(wlroots.backend, ApplyBackendSetting::Wlroots);
    assert_eq!(hyprland.backend, ApplyBackendSetting::Hyprland);
}

#[test]
fn maps_fill_modes_to_wlroots_modes() {
    assert_eq!(wlroots_scale_mode(FillMode::Os), "fill");
    assert_eq!(wlroots_scale_mode(FillMode::Zoom), "fill");
    assert_eq!(wlroots_scale_mode(FillMode::Spanned), "fill");
    assert_eq!(wlroots_scale_mode(FillMode::Centered), "center");
    assert_eq!(wlroots_scale_mode(FillMode::Scaled), "fit");
    assert_eq!(wlroots_scale_mode(FillMode::Stretched), "stretch");
    assert_eq!(wlroots_scale_mode(FillMode::Wallpaper), "tile");
}

#[test]
fn sway_output_bg_args_set_all_outputs() {
    assert_eq!(
        sway_output_bg_args(
            Path::new("/tmp/New Caledonia (ID-1240).jpg"),
            FillMode::Zoom
        ),
        vec![
            "output",
            "*",
            "bg",
            "/tmp/New Caledonia (ID-1240).jpg",
            "fill"
        ]
    );
}

#[test]
fn hyprctl_monitors_args_lists_monitors_without_json_or_jq() {
    assert_eq!(hyprctl_monitors_args(), vec!["monitors"]);
}

#[test]
fn hyprland_monitor_names_parse_text_output() {
    let monitors = hyprland_monitor_names(
        br#"
Monitor DP-1 (ID 0):
	3840x2160@59.99700 at 0x0
Monitor HDMI-A-1 (ID 1):
	1920x1080@60.00000 at 3840x0
"#,
    );

    assert_eq!(monitors, vec!["DP-1", "HDMI-A-1"]);
}

#[test]
fn wlroots_swaybg_commands_target_each_monitor() {
    let commands = wlroots_swaybg_commands(
        &["DP-1".to_string(), "HDMI-A-1".to_string()],
        Path::new("/tmp/wall.jpg"),
        FillMode::Scaled,
    );

    assert_eq!(
        commands,
        vec![
            vec!["-o", "DP-1", "-i", "/tmp/wall.jpg", "-m", "fit"],
            vec!["-o", "HDMI-A-1", "-i", "/tmp/wall.jpg", "-m", "fit"],
        ]
    );
}

#[test]
fn wlroots_swaybg_commands_fall_back_to_all_outputs() {
    assert_eq!(
        wlroots_swaybg_commands(&[], Path::new("/tmp/wall.jpg"), FillMode::Centered),
        vec![vec!["-i", "/tmp/wall.jpg", "-m", "center"]]
    );
}

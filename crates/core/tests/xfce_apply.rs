use std::path::Path;

use walls_core::apply::{
    connected_xrandr_monitors, detect_desktop_from_env, xfce_existing_backdrop_properties,
    xfce_existing_property_commands, xfce_list_backdrop_args, xfce_new_monitor_commands, Desktop,
};
use walls_core::config::{ApplyBackendSetting, ApplyConfig};

#[test]
fn detects_xfce_from_xdg_current_desktop() {
    assert_eq!(
        detect_desktop_from_env(Some("XFCE"), None, None),
        Desktop::Xfce
    );
    assert_eq!(
        detect_desktop_from_env(Some("X-Cinnamon:XFCE"), None, None),
        Desktop::Xfce
    );
}

#[test]
fn parses_explicit_xfce_backend_config() {
    let apply: ApplyConfig = serde_json::from_str(r#"{"backend":"xfce"}"#).unwrap();
    assert_eq!(apply.backend, ApplyBackendSetting::Xfce);
}

#[test]
fn xfce_list_backdrop_args_lists_backdrop_root() {
    assert_eq!(
        xfce_list_backdrop_args(),
        vec!["-c", "xfce4-desktop", "-p", "/backdrop", "-l"]
    );
}

#[test]
fn xfce_existing_backdrop_properties_keep_image_paths_and_last_images() {
    let props = xfce_existing_backdrop_properties(
        br#"
/backdrop/screen0/monitorDP-1/workspace0/last-image
/backdrop/screen0/monitorDP-1/workspace0/image-style
/backdrop/screen0/monitorHDMI-1/workspace0/image-path
/other/screen0/monitorHDMI-1/workspace0/last-image
"#,
    );

    assert_eq!(
        props,
        vec![
            "/backdrop/screen0/monitorDP-1/workspace0/last-image",
            "/backdrop/screen0/monitorHDMI-1/workspace0/image-path",
        ]
    );
}

#[test]
fn connected_xrandr_monitors_returns_connected_outputs_only() {
    let monitors = connected_xrandr_monitors(
        br#"
DP-1 connected primary 3840x2160+0+0
HDMI-1 disconnected
eDP-1 connected 1920x1080+3840+0
"#,
    );

    assert_eq!(monitors, vec!["DP-1", "eDP-1"]);
}

#[test]
fn xfce_existing_property_commands_clear_then_set_each_property() {
    let props = vec![
        "/backdrop/screen0/monitorDP-1/workspace0/last-image".to_string(),
        "/backdrop/screen0/monitorHDMI-1/workspace0/image-path".to_string(),
    ];
    let commands =
        xfce_existing_property_commands(&props, Path::new("/tmp/New Caledonia (ID-1240).jpg"));

    assert_eq!(commands.len(), 6);
    assert_eq!(
        commands[0],
        vec![
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitorDP-1/workspace0/last-image",
            "-n",
            "-t",
            "string",
            "-s",
            ""
        ]
    );
    assert_eq!(
        commands[2],
        vec![
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitorDP-1/workspace0/last-image",
            "-s",
            "/tmp/New Caledonia (ID-1240).jpg"
        ]
    );
    assert_eq!(
        commands[5],
        vec![
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitorHDMI-1/workspace0/image-path",
            "-s",
            "/tmp/New Caledonia (ID-1240).jpg"
        ]
    );
}

#[test]
fn xfce_new_monitor_commands_create_last_image_and_zoom_style() {
    let commands = xfce_new_monitor_commands(
        &["DP-1".to_string(), "eDP-1".to_string()],
        Path::new("/tmp/wall.jpg"),
    );

    assert_eq!(
        commands,
        vec![
            vec![
                "-c",
                "xfce4-desktop",
                "-p",
                "/backdrop/screen0/monitorDP-1/workspace0/last-image",
                "-n",
                "-t",
                "string",
                "-s",
                "/tmp/wall.jpg",
            ],
            vec![
                "-c",
                "xfce4-desktop",
                "-p",
                "/backdrop/screen0/monitorDP-1/workspace0/image-style",
                "-n",
                "-t",
                "int",
                "-s",
                "5",
            ],
            vec![
                "-c",
                "xfce4-desktop",
                "-p",
                "/backdrop/screen0/monitoreDP-1/workspace0/last-image",
                "-n",
                "-t",
                "string",
                "-s",
                "/tmp/wall.jpg",
            ],
            vec![
                "-c",
                "xfce4-desktop",
                "-p",
                "/backdrop/screen0/monitoreDP-1/workspace0/image-style",
                "-n",
                "-t",
                "int",
                "-s",
                "5",
            ],
        ]
    );
}

#[test]
fn xfce_new_monitor_commands_fall_back_to_default_monitor_property() {
    let commands = xfce_new_monitor_commands(&[], Path::new("/tmp/wall.jpg"));

    assert_eq!(
        commands,
        vec![vec![
            "-c",
            "xfce4-desktop",
            "-p",
            "/backdrop/screen0/monitor0/workspace0/last-image",
            "-n",
            "-t",
            "string",
            "-s",
            "/tmp/wall.jpg",
        ]]
    );
}

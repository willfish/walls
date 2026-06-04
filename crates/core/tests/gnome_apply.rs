use std::path::Path;

use walls_core::apply::{detect_desktop_from_env, gnome_gsettings_commands, Desktop, FillMode};
use walls_core::config::{ApplyBackendSetting, ApplyConfig};

#[test]
fn detects_gnome_family_from_xdg_current_desktop() {
    assert_eq!(
        detect_desktop_from_env(Some("GNOME"), None, None),
        Desktop::Gnome
    );
    assert_eq!(
        detect_desktop_from_env(Some("Unity:Unity7"), None, None),
        Desktop::Unity
    );
    assert_eq!(
        detect_desktop_from_env(Some("Budgie:GNOME"), None, None),
        Desktop::Budgie
    );
}

#[test]
fn parses_explicit_gnome_backend_config() {
    let apply: ApplyConfig = serde_json::from_str(r#"{"backend":"gnome"}"#).unwrap();
    assert_eq!(apply.backend, ApplyBackendSetting::Gnome);
}

#[test]
fn gnome_commands_set_light_dark_uri_and_picture_options() {
    let commands = gnome_gsettings_commands(
        Path::new("/tmp/New Caledonia (ID-1240).jpg"),
        FillMode::Zoom,
    );

    assert_eq!(
        commands[0],
        vec![
            "set",
            "org.gnome.desktop.background",
            "picture-uri",
            "file:///tmp/New%20Caledonia%20%28ID-1240%29.jpg"
        ]
    );
    assert_eq!(
        commands[1],
        vec![
            "set",
            "org.gnome.desktop.background",
            "picture-uri-dark",
            "file:///tmp/New%20Caledonia%20%28ID-1240%29.jpg"
        ]
    );
    assert_eq!(
        commands[2],
        vec![
            "set",
            "org.gnome.desktop.background",
            "picture-options",
            "zoom"
        ]
    );
}

#[test]
fn gnome_commands_leave_picture_options_alone_for_os_mode() {
    let commands = gnome_gsettings_commands(Path::new("/tmp/wall.jpg"), FillMode::Os);

    assert_eq!(commands.len(), 2);
}

use std::path::Path;

use walls_core::apply::{
    detect_desktop_from_env, kde_dbus_send_args, unsupported_plugins_from_dbus_reply, Desktop,
};
use walls_core::config::{ApplyBackendSetting, ApplyConfig};

#[test]
fn detects_kde_from_xdg_current_desktop() {
    assert_eq!(
        detect_desktop_from_env(Some("KDE"), None, None),
        Desktop::Kde
    );
    assert_eq!(
        detect_desktop_from_env(Some("plasma:KDE"), None, None),
        Desktop::Kde
    );
}

#[test]
fn parses_explicit_kde_backend_config() {
    let apply: ApplyConfig = serde_json::from_str(r#"{"backend":"kde"}"#).unwrap();
    assert_eq!(apply.backend, ApplyBackendSetting::Kde);
}

#[test]
fn kde_dbus_send_args_evaluate_plasma_script() {
    let args = kde_dbus_send_args(Path::new("/tmp/New Caledonia (ID-1240).jpg"));

    assert_eq!(
        &args[..5],
        [
            "--print-reply",
            "--type=method_call",
            "--dest=org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
        ]
    );
    assert!(args[5].starts_with("string:"));
    assert!(args[5].contains("desktops()"));
    assert!(args[5].contains("org.kde.image"));
    assert!(args[5].contains("a2n.blur"));
    assert!(args[5]
        .contains("d.writeConfig('Image', \"file:///tmp/New%20Caledonia%20%28ID-1240%29.jpg\")"));
}

#[test]
fn parses_unsupported_plugins_from_dbus_reply() {
    let stdout = br#"method return time=1710000000.000 sender=:1.12 -> destination=:1.24 serial=42 reply_serial=2
   string "org.example.unsupported"
"#;

    assert_eq!(
        unsupported_plugins_from_dbus_reply(stdout),
        Some("org.example.unsupported".into())
    );
}

#[test]
fn missing_dbus_string_reply_is_not_an_unsupported_plugin_error() {
    assert_eq!(unsupported_plugins_from_dbus_reply(b""), None);
}

//! Regression tests for `walls tui` terminal handling.
//!
//! Without a TTY, crossterm raw mode used to panic (ENXIO). We require a TTY up front
//! and must keep failing cleanly when stdin/stdout are piped or detached.

use std::fs;
use std::io::Write;
use std::process::{Command as StdCommand, Stdio};

use assert_cmd::cargo::cargo_bin;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

fn setup_xdg_home(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let config_home = tmp.join("xdg-config");
    let state_home = tmp.join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("a.jpg"), b"x").unwrap();

    let noop = tmp.join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": tmp.join("cache").display().to_string(),
            "download_dir": tmp.join("downloaded").display().to_string(),
            "favorites_dir": tmp.join("favorites").display().to_string(),
            "fetched_dir": tmp.join("fetched").display().to_string(),
            "compose_dir": tmp.join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "sources": [{ "enabled": true, "type": "folder", "path": images.display().to_string() }],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();
    (config_home, state_home)
}

#[test]
fn tui_without_tty_returns_clear_error_not_panic() {
    let output = StdCommand::new(cargo_bin("walls"))
        .arg("tui")
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run walls tui without tty");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_ascii_lowercase().contains("terminal")
            || stderr.to_ascii_lowercase().contains("tty")
            || stderr.to_ascii_lowercase().contains("stdin")
            || stderr.to_ascii_lowercase().contains("stdout")
            || stderr.to_ascii_lowercase().contains("interactive"),
        "expected TTY hint on stderr, got: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
    assert!(!stderr.contains("ENXIO"), "must not hit raw-mode ENXIO: {stderr}");
}

#[test]
fn bare_walls_without_tty_does_not_launch_tui() {
    let output = StdCommand::new(cargo_bin("walls"))
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run walls without tty");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no command specified"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
}

#[test]
fn tui_with_pty_exits_cleanly_on_quit() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");

    let mut cmd = CommandBuilder::new(cargo_bin("walls"));
    cmd.arg("tui");
    cmd.env("XDG_CONFIG_HOME", config_home);
    cmd.env("XDG_STATE_HOME", state_home);
    cmd.env("RUST_BACKTRACE", "0");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn walls tui in pty");
    drop(pair.slave);

    let mut writer = pair.master.take_writer().expect("pty writer");
    std::thread::sleep(std::time::Duration::from_millis(400));
    writer.write_all(b"q").expect("send quit");
    drop(writer);

    let status = child.wait().expect("wait for walls tui");
    assert!(status.success(), "walls tui should exit 0 after q, got {status:?}");
}
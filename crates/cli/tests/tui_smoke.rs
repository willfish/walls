//! Regression tests for TUI launch via bare `walls` (default, no subcommand) or explicit `walls tui`.
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
    fs::write(images.join("b.jpg"), b"y").unwrap(); // 2 images so 'n' can switch to a "different" one

    let noop = tmp.join("noop.sh");
    // Make the "noop" a logger so we can observe that 'n' actually caused a real wallpaper apply/switch.
    // It logs the original path arg ($3 in the custom script call) and succeeds (so TUI doesn't error).
    let applied_log = tmp.join("applied.log");
    fs::write(
        &noop,
        format!(
            "#!/bin/sh\necho \"APPLIED:$3\" >> \"{}\"\nexit 0\n",
            applied_log.display()
        ),
    )
    .unwrap();
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
        .expect("run walls tui (explicit subcmd) without tty");

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
    assert!(
        !stderr.contains("ENXIO"),
        "must not hit raw-mode ENXIO: {stderr}"
    );
}

#[test]
fn bare_walls_without_tty_attempts_tui_and_reports_terminal_requirement() {
    // Bare `walls` (no subcommand/args) must default to starting the TUI.
    // Without a TTY, tui::run() -> require_tty() must fail cleanly with terminal hint
    // (same behavior and error as explicit `walls tui`), not the old "no command specified".
    let output = StdCommand::new(cargo_bin("walls"))
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run walls without tty");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_ascii_lowercase().contains("terminal")
            || stderr.to_ascii_lowercase().contains("tty")
            || stderr.to_ascii_lowercase().contains("stdin")
            || stderr.to_ascii_lowercase().contains("stdout")
            || stderr.to_ascii_lowercase().contains("interactive"),
        "expected TTY requirement hint on stderr for bare default-to-tui, got: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "must not panic: {stderr}");
    assert!(
        !stderr.contains("ENXIO"),
        "must not hit raw-mode ENXIO: {stderr}"
    );
    // Prove we no longer emit the old no-command message for bare (default is now TUI)
    assert!(
        !stderr.contains("no command specified"),
        "should not fall back to 'no command' for bare invocation: {stderr}"
    );
}

#[test]
fn tui_with_pty_exits_cleanly_on_quit() {
    // buildRustPackage / Nix sandbox: cargo_bin path is not always spawnable via portable-pty.
    if std::env::var_os("NIX_BUILD_TOP").is_some() {
        return;
    }

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
    // Intentionally no "tui" arg: bare `walls` must default to TUI launch (the primary UX)
    cmd.env("XDG_CONFIG_HOME", config_home);
    cmd.env("XDG_STATE_HOME", state_home);
    cmd.env("RUST_BACKTRACE", "0");
    cmd.env("WALLS_TUI_PREVIEW", "0");

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .expect("spawn walls (bare default) in pty");
    drop(pair.slave);

    let mut writer = pair.master.take_writer().expect("pty writer");
    std::thread::sleep(std::time::Duration::from_millis(400));
    // Actually try switching to the next wallpaper with real 'n' key (twice) in pty-driven TUI.
    // This exercises ... and verifies *actual change* to a different wallpaper (not just script called with possibly same path).
    // The setup has 2 distinct images; second 'n' should cause apply of the other (thanks to avoid_recent + picker).
    writer
        .write_all(b"n")
        .expect("send n to actually switch to next wallpaper");
    std::thread::sleep(std::time::Duration::from_millis(500));
    writer
        .write_all(b"n")
        .expect("send second n to switch to a different wallpaper");
    std::thread::sleep(std::time::Duration::from_millis(500));
    writer.write_all(b"q").expect("send quit");
    drop(writer);

    let status = child.wait().expect("wait for walls bare default TUI");
    assert!(
        status.success(),
        "walls (bare) should exit 0 after n+q, got {status:?}"
    );

    // Evidence that 'n' actually caused *wallpaper changes* (different images applied on consecutive n, not re-apply of same).
    // This is the proper verification that "log says applies" also means the wallpaper actually switched (addresses user report where cosmic patch/script logged but no visual change until native switcher normalized the config).
    let applied_log = tmp.path().join("applied.log");
    let log_content = fs::read_to_string(&applied_log).unwrap_or_default();
    let applied_lines: Vec<&str> = log_content
        .lines()
        .filter(|l| l.contains("APPLIED:"))
        .collect();
    assert!(
        applied_lines.len() >= 2,
        "two 'n' should have caused at least 2 apply calls; log: {}",
        log_content
    );
    // The two applied originals should be different (a.jpg and b.jpg in setup).
    assert!(
        log_content.contains("a.jpg") && log_content.contains("b.jpg"),
        "consecutive 'n' must have switched between the two different images; log: {}",
        log_content
    );
}

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;
use walls_core::config::DisplayConfig;
use walls_core::paths::WallsPaths;
use walls_core::pipeline::compose;

#[test]
fn compose_leaves_display_mode_to_backend_when_target_size_is_missing() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        mode: "fill-with-blur".into(),
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_eq!(composed, original);
    assert!(!paths.compose_dir.exists());
}

#[test]
fn compose_runs_zoom_display_mode_for_configured_target_size() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    let log = temp.path().join("display-mode.log");
    let command = display_mode_script(temp.path(), &log);
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        mode: "zoom".into(),
        imagemagick_command: command.display().to_string(),
        target_width: Some(1920),
        target_height: Some(1080),
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_ne!(composed, original);
    assert_eq!(composed.extension().and_then(|s| s.to_str()), Some("png"));
    assert!(composed.exists());
    assert!(composed.starts_with(&paths.compose_dir));
    assert!(composed
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap()
        .contains(".zoom.1920x1080.png"));

    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains(&original.display().to_string()));
    assert!(log.contains("-resize"));
    assert!(log.contains("1920x1080^"));
    assert!(log.contains("-extent"));
    assert!(log.contains("1920x1080"));
    assert!(log.contains(&composed.display().to_string()));
}

#[test]
fn compose_runs_blur_pad_display_mode_for_configured_target_size() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    let log = temp.path().join("display-mode.log");
    let command = display_mode_script(temp.path(), &log);
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        mode: "fill-with-blur".into(),
        imagemagick_command: command.display().to_string(),
        target_width: Some(1600),
        target_height: Some(900),
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_ne!(composed, original);
    assert!(composed
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap()
        .contains(".fill-with-blur.1600x900.png"));

    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains("-clone"));
    assert!(log.contains("-blur"));
    assert!(log.contains("0x16"));
    assert!(log.contains("-composite"));
}

fn test_paths(temp: &TempDir) -> WallsPaths {
    let root = temp.path();
    WallsPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.json"),
        secrets_file: root.join("config/secrets.json"),
        state_file: root.join("state/state.json"),
        event_journal_file: root.join("state/events.jsonl"),
        cache_dir: root.join("cache"),
        download_dir: root.join("downloaded"),
        favorites_dir: root.join("favorites"),
        fetched_dir: root.join("fetched"),
        compose_dir: root.join("wallpaper"),
    }
}

fn display_mode_script(root: &std::path::Path, log: &std::path::Path) -> std::path::PathBuf {
    let script = root.join("display-mode.sh");
    write_script(
        &script,
        &format!(
            r#"#!/bin/sh
input=$1
last=
for arg in "$@"; do
  last=$arg
done
printf '%s\n' "$@" > '{}'
cp "$input" "$last"
"#,
            log.display()
        ),
    );
    script
}

fn write_script(path: &std::path::Path, contents: &str) {
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);
    make_executable(&tmp);
    fs::rename(tmp, path).unwrap();
}

fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

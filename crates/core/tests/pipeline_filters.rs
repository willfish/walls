use std::fs;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;
use walls_core::config::{DisplayConfig, DisplayFiltersConfig, ImageMagickFilterConfig};
use walls_core::paths::WallsPaths;
use walls_core::pipeline::compose;

#[test]
fn compose_returns_original_when_filter_list_is_empty() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        filters: DisplayFiltersConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_eq!(composed, original);
    assert!(!paths.compose_dir.exists());
}

#[test]
fn compose_runs_one_configured_imagemagick_filter() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    let log = temp.path().join("filter.log");
    let command = filter_script(temp.path(), &log);
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        filters: DisplayFiltersConfig {
            enabled: true,
            command: command.display().to_string(),
            filters: vec![ImageMagickFilterConfig {
                name: "Sepia Tone".into(),
                args: vec!["-sepia-tone".into(), "80%".into()],
            }],
        },
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
        .contains(".sepia-tone.png"));

    let log = fs::read_to_string(log).unwrap();
    assert!(log.contains(&original.display().to_string()));
    assert!(log.contains("-sepia-tone"));
    assert!(log.contains("80%"));
    assert!(log.contains(&composed.display().to_string()));
}

#[test]
fn compose_reports_failed_imagemagick_filter() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("wallpaper.jpg");
    let command = failing_script(temp.path());
    fs::write(&original, b"image").unwrap();

    let display = DisplayConfig {
        filters: DisplayFiltersConfig {
            enabled: true,
            command: command.display().to_string(),
            filters: vec![ImageMagickFilterConfig {
                name: "bad filter".into(),
                args: vec!["-bad".into()],
            }],
        },
        ..Default::default()
    };

    let err = compose(&paths, &display, &original).unwrap_err();

    assert!(
        err.to_string()
            .contains("ImageMagick filter 'bad filter' failed"),
        "{err}"
    );
}

fn test_paths(temp: &TempDir) -> WallsPaths {
    let root = temp.path();
    WallsPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.json"),
        secrets_file: root.join("config/secrets.json"),
        state_file: root.join("state/state.json"),
        cache_dir: root.join("cache"),
        download_dir: root.join("downloaded"),
        favorites_dir: root.join("favorites"),
        fetched_dir: root.join("fetched"),
        compose_dir: root.join("wallpaper"),
    }
}

fn filter_script(root: &std::path::Path, log: &std::path::Path) -> std::path::PathBuf {
    let script = root.join("filter.sh");
    fs::write(
        &script,
        format!(
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
    )
    .unwrap();
    make_executable(&script);
    script
}

fn failing_script(root: &std::path::Path) -> std::path::PathBuf {
    let script = root.join("filter-fail.sh");
    fs::write(&script, "#!/bin/sh\nexit 7\n").unwrap();
    make_executable(&script);
    script
}

fn make_executable(path: &std::path::Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

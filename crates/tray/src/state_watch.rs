//! Detect wallpaper/state changes from other walls processes (e.g. the TUI).

use std::fs;
use std::time::SystemTime;

use walls_core::config::{load_or_create_config, TrayAccent};
use walls_core::cosmic_theme;
use walls_core::paths::{expand_home, WallsPaths};
use walls_core::rotation::rotation_inactive;
use walls_core::state::State;
use walls_core::tray_icon::effective_tray_accent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayVisualState {
    pub composed_path: Option<String>,
    pub rotation_inactive: bool,
    pub tray_accent: TrayAccent,
    pub cosmic_theme_mtime: Option<SystemTime>,
    pub image_mtime: Option<SystemTime>,
}

impl TrayVisualState {
    pub fn load(paths: &WallsPaths) -> anyhow::Result<Self> {
        let state = State::load_or_default(&paths.state_file)?;
        let config = load_or_create_config(&paths.config_file)?;
        let inactive = rotation_inactive(&state, &config);
        let composed_path = state
            .current
            .as_ref()
            .map(|current| current.composed_path.clone());
        let image_mtime = composed_path
            .as_deref()
            .map(expand_home)
            .and_then(|path| fs::metadata(path).ok())
            .and_then(|meta| meta.modified().ok());
        let cosmic_theme_mtime = if effective_tray_accent(config.tray.accent) == TrayAccent::Cosmic
        {
            cosmic_theme::cosmic_theme_stamp()
        } else {
            None
        };
        Ok(Self {
            composed_path,
            rotation_inactive: inactive,
            tray_accent: config.tray.accent,
            cosmic_theme_mtime,
            image_mtime,
        })
    }
}

pub struct StateWatcher {
    paths: WallsPaths,
    last_visual: TrayVisualState,
}

impl StateWatcher {
    pub fn new() -> anyhow::Result<Self> {
        let paths = WallsPaths::discover()?;
        let last_visual = TrayVisualState::load(&paths)?;
        Ok(Self { paths, last_visual })
    }

    /// Returns `true` when the tray icon or tooltip should refresh.
    pub fn poll(&mut self) -> bool {
        let Ok(visual) = TrayVisualState::load(&self.paths) else {
            return false;
        };
        if visual == self.last_visual {
            return false;
        }
        self.last_visual = visual;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;
    use walls_core::config::{load_or_create_config, Config};
    use walls_core::paths::WallsPaths;
    use walls_core::state::{CurrentWall, State};

    fn test_paths(root: &Path) -> WallsPaths {
        WallsPaths {
            config_dir: root.to_path_buf(),
            config_file: root.join("config.json"),
            secrets_file: root.join("secrets.json"),
            state_file: root.join("state.json"),
            cache_dir: root.join("cache"),
            download_dir: root.join("downloaded"),
            favorites_dir: root.join("favorites"),
            fetched_dir: root.join("fetched"),
            compose_dir: root.join("wallpaper"),
        }
    }

    fn write_state(path: &Path, state: &State) {
        state.save(path).unwrap();
    }

    fn write_config(path: &Path, config: &Config) {
        let data = serde_json::to_string_pretty(config).unwrap();
        fs::write(path, data).unwrap();
    }

    fn sample_state(composed_path: &str, paused: bool) -> State {
        State {
            paused,
            current: Some(CurrentWall {
                source_id: "test".into(),
                wallhaven_id: None,
                provider: None,
                source_url: None,
                author: None,
                description: None,
                original_path: composed_path.into(),
                composed_path: composed_path.into(),
                post_filter_path: None,
            }),
            last_change_unix: 1,
            ..State::default()
        }
    }

    fn setup_watcher(root: &Path, state: &State) -> StateWatcher {
        let paths = test_paths(root);
        load_or_create_config(&paths.config_file).unwrap();
        write_state(&paths.state_file, state);
        let last_visual = TrayVisualState::load(&paths).unwrap();
        StateWatcher { paths, last_visual }
    }

    #[test]
    fn poll_is_false_until_wallpaper_changes() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));

        assert!(!watcher.poll());

        std::thread::sleep(Duration::from_millis(20));
        write_state(
            &watcher.paths.state_file,
            &sample_state("/tmp/b.jpg", false),
        );
        assert!(watcher.poll());
        assert!(!watcher.poll());
    }

    #[test]
    fn poll_ignores_queue_only_state_updates() {
        let root = tempfile::tempdir().unwrap();
        let mut state = sample_state("/tmp/a.jpg", false);
        let mut watcher = setup_watcher(root.path(), &state);

        state.cache_queue.push("wh:123".into());
        state.last_change_unix = 2;
        std::thread::sleep(Duration::from_millis(20));
        write_state(&watcher.paths.state_file, &state);

        assert!(!watcher.poll());
    }

    #[test]
    fn poll_detects_pause_toggle() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));

        write_state(&watcher.paths.state_file, &sample_state("/tmp/a.jpg", true));
        assert!(watcher.poll());
    }

    #[test]
    fn poll_detects_rotation_disabled_in_config() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));

        let mut config = load_or_create_config(&watcher.paths.config_file).unwrap();
        config.change.enabled = false;
        write_config(&watcher.paths.config_file, &config);
        assert!(watcher.poll());
    }

    #[test]
    fn poll_detects_tray_accent_change() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));

        let mut config = load_or_create_config(&watcher.paths.config_file).unwrap();
        config.tray.accent = walls_core::config::TrayAccent::Green;
        write_config(&watcher.paths.config_file, &config);
        assert!(watcher.poll());
    }

    #[test]
    fn poll_detects_cosmic_theme_file_change() {
        let _lock = cosmic_theme::lock_env_for_tests();
        std::env::set_var("XDG_CURRENT_DESKTOP", "COSMIC");
        let root = tempfile::tempdir().unwrap();
        let xdg = root.path().join("xdg");
        let cosmic_config = xdg.join("cosmic");
        fs::create_dir_all(cosmic_config.join("com.system76.CosmicTheme.Mode/v1")).unwrap();
        fs::create_dir_all(cosmic_config.join("com.system76.CosmicTheme.Dark/v1")).unwrap();
        fs::write(
            cosmic_config.join("com.system76.CosmicTheme.Mode/v1/is_dark"),
            "true",
        )
        .unwrap();
        let accent_path = cosmic_config.join("com.system76.CosmicTheme.Dark/v1/accent");
        fs::write(
            &accent_path,
            r"(
    base: ( red: 0.1, green: 0.2, blue: 0.3, alpha: 1.0, ),
    hover: ( red: 0.2, green: 0.3, blue: 0.4, alpha: 1.0, ),
    focus: ( red: 0.3, green: 0.4, blue: 0.5, alpha: 1.0, ),
    border: ( red: 0.4, green: 0.5, blue: 0.6, alpha: 1.0, ),
)",
        )
        .unwrap();

        std::env::set_var("XDG_CONFIG_HOME", &xdg);

        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));
        let mut config = load_or_create_config(&watcher.paths.config_file).unwrap();
        config.tray.accent = walls_core::config::TrayAccent::Cosmic;
        write_config(&watcher.paths.config_file, &config);
        assert!(watcher.poll());

        std::thread::sleep(Duration::from_millis(50));
        fs::write(
            &accent_path,
            r"(
    base: ( red: 0.5, green: 0.6, blue: 0.7, alpha: 1.0, ),
    hover: ( red: 0.6, green: 0.7, blue: 0.8, alpha: 1.0, ),
    focus: ( red: 0.7, green: 0.8, blue: 0.9, alpha: 1.0, ),
    border: ( red: 0.8, green: 0.9, blue: 1.0, alpha: 1.0, ),
)",
        )
        .unwrap();

        std::thread::sleep(Duration::from_millis(50));
        assert!(watcher.poll());

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CURRENT_DESKTOP");
    }

    #[test]
    fn poll_detects_when_all_sources_disabled() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = setup_watcher(root.path(), &sample_state("/tmp/a.jpg", false));

        let mut config = load_or_create_config(&watcher.paths.config_file).unwrap();
        for source in &mut config.sources {
            source.enabled = false;
        }
        config.wallhaven.enabled = false;
        write_config(&watcher.paths.config_file, &config);
        assert!(watcher.poll());
    }

    #[test]
    fn poll_detects_when_composed_image_file_appears() {
        let root = tempfile::tempdir().unwrap();
        let image_path = root.path().join("wall.png");
        let mut watcher = setup_watcher(
            root.path(),
            &sample_state(&image_path.display().to_string(), false),
        );
        assert!(!watcher.poll());

        std::thread::sleep(Duration::from_millis(20));
        fs::write(&image_path, b"png").unwrap();
        assert!(watcher.poll());
    }
}

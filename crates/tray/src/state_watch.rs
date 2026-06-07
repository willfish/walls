//! Detect wallpaper/state changes from other walls processes (e.g. the TUI).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walls_core::paths::{expand_home, WallsPaths};
use walls_core::state::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayVisualState {
    pub composed_path: Option<String>,
    pub paused: bool,
    pub image_mtime: Option<SystemTime>,
}

impl TrayVisualState {
    pub fn load(state_file: &Path) -> anyhow::Result<Self> {
        let state = State::load_or_default(state_file)?;
        let composed_path = state.current.map(|current| current.composed_path);
        let image_mtime = composed_path
            .as_deref()
            .map(expand_home)
            .and_then(|path| fs::metadata(path).ok())
            .and_then(|meta| meta.modified().ok());
        Ok(Self {
            composed_path,
            paused: state.paused,
            image_mtime,
        })
    }
}

pub struct StateWatcher {
    state_file: PathBuf,
    last_visual: TrayVisualState,
}

impl StateWatcher {
    pub fn new() -> anyhow::Result<Self> {
        let paths = WallsPaths::discover()?;
        let state_file = paths.state_file;
        let last_visual = TrayVisualState::load(&state_file)?;
        Ok(Self {
            state_file,
            last_visual,
        })
    }

    /// Returns `true` when the tray icon or tooltip should refresh.
    pub fn poll(&mut self) -> bool {
        let Ok(visual) = TrayVisualState::load(&self.state_file) else {
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
    use std::time::Duration;
    use walls_core::state::{CurrentWall, State};

    fn write_state(path: &Path, state: &State) {
        state.save(path).unwrap();
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

    #[test]
    fn poll_is_false_until_wallpaper_changes() {
        let root = tempfile::tempdir().unwrap();
        let state_file = root.path().join("state.json");
        write_state(&state_file, &sample_state("/tmp/a.jpg", false));

        let mut watcher = StateWatcher {
            state_file: state_file.clone(),
            last_visual: TrayVisualState::load(&state_file).unwrap(),
        };

        assert!(!watcher.poll());

        std::thread::sleep(Duration::from_millis(20));
        write_state(&state_file, &sample_state("/tmp/b.jpg", false));
        assert!(watcher.poll());
        assert!(!watcher.poll());
    }

    #[test]
    fn poll_ignores_queue_only_state_updates() {
        let root = tempfile::tempdir().unwrap();
        let state_file = root.path().join("state.json");
        let mut state = sample_state("/tmp/a.jpg", false);
        write_state(&state_file, &state);

        let mut watcher = StateWatcher {
            state_file: state_file.clone(),
            last_visual: TrayVisualState::load(&state_file).unwrap(),
        };

        state.cache_queue.push("wh:123".into());
        state.last_change_unix = 2;
        std::thread::sleep(Duration::from_millis(20));
        write_state(&state_file, &state);

        assert!(!watcher.poll());
    }

    #[test]
    fn poll_detects_pause_toggle() {
        let root = tempfile::tempdir().unwrap();
        let state_file = root.path().join("state.json");
        write_state(&state_file, &sample_state("/tmp/a.jpg", false));

        let mut watcher = StateWatcher {
            state_file: state_file.clone(),
            last_visual: TrayVisualState::load(&state_file).unwrap(),
        };

        write_state(&state_file, &sample_state("/tmp/a.jpg", true));
        assert!(watcher.poll());
    }

    #[test]
    fn poll_detects_when_composed_image_file_appears() {
        let root = tempfile::tempdir().unwrap();
        let image_path = root.path().join("wall.png");
        let state_file = root.path().join("state.json");
        write_state(
            &state_file,
            &sample_state(&image_path.display().to_string(), false),
        );

        let mut watcher = StateWatcher {
            state_file: state_file.clone(),
            last_visual: TrayVisualState::load(&state_file).unwrap(),
        };
        assert!(!watcher.poll());

        std::thread::sleep(Duration::from_millis(20));
        fs::write(&image_path, b"png").unwrap();
        assert!(watcher.poll());
    }
}

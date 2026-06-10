use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;

use walls_core::preview_cache::{
    previewable_paths_from_state, prewarm_preview_thumbnails, DEFAULT_PREVIEW_SIZE,
};
use walls_core::state::State;
use walls_core::WallsCtx;

pub struct PreviewPrewarmer {
    paths: PreviewPrewarmPaths,
    tx: SyncSender<PreviewPrewarmJob>,
    last_fingerprint: Option<u64>,
}

#[derive(Clone)]
struct PreviewPrewarmPaths {
    state_file: std::path::PathBuf,
    cache_dir: std::path::PathBuf,
}

struct PreviewPrewarmJob {
    state: State,
}

impl PreviewPrewarmer {
    pub fn new() -> anyhow::Result<Self> {
        let ctx = WallsCtx::load_without_autostart_sync()?;
        let paths = preview_prewarm_paths_from_ctx(ctx);
        let (tx, rx) = mpsc::sync_channel::<PreviewPrewarmJob>(1);
        let worker_paths = paths.clone();
        thread::Builder::new()
            .name("walls-preview-prewarm".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    let sources = previewable_paths_from_state(&job.state, &worker_paths.cache_dir);
                    if sources.is_empty() {
                        continue;
                    }
                    let stats = prewarm_preview_thumbnails(
                        sources,
                        &worker_paths.cache_dir,
                        DEFAULT_PREVIEW_SIZE,
                    );
                    if stats.attempted > 0 {
                        tracing::debug!(
                            "preview prewarm: attempted={} warmed={} failed={}",
                            stats.attempted,
                            stats.warmed,
                            stats.failed
                        );
                    }
                }
            })?;

        Ok(Self {
            paths,
            tx,
            last_fingerprint: None,
        })
    }

    pub fn poll(&mut self) {
        let Ok(state) = State::load_or_default(&self.paths.state_file) else {
            return;
        };
        let fingerprint = preview_fingerprint(&state);
        if self.last_fingerprint == Some(fingerprint) {
            return;
        }

        match self.tx.try_send(PreviewPrewarmJob { state }) {
            Ok(()) => self.last_fingerprint = Some(fingerprint),
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => self.last_fingerprint = Some(fingerprint),
        }
    }
}

fn preview_prewarm_paths_from_ctx(ctx: WallsCtx) -> PreviewPrewarmPaths {
    PreviewPrewarmPaths {
        state_file: ctx.paths.state_file,
        cache_dir: ctx.paths.cache_dir,
    }
}

fn preview_fingerprint(state: &State) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.cache_queue.hash(&mut hasher);
    state.history.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use walls_core::config::{default_config, Secrets};
    use walls_core::paths::WallsPaths;
    use walls_core::providers::ProviderStatusReport;

    #[test]
    fn preview_fingerprint_tracks_queue_and_history_changes() {
        let mut state = State::default();
        let empty = preview_fingerprint(&state);

        state.cache_queue.push("wh:one".into());
        let with_queue = preview_fingerprint(&state);

        state.history.push("/tmp/wall.jpg".into());
        let with_history = preview_fingerprint(&state);

        assert_ne!(empty, with_queue);
        assert_ne!(with_queue, with_history);
    }

    #[test]
    fn preview_fingerprint_ignores_unrelated_state_fields() {
        let mut state = State {
            paused: true,
            no_effects_on: Some("2026-06-10".into()),
            history_index: 2,
            last_change_unix: 42,
            ..State::default()
        };
        let fingerprint = preview_fingerprint(&state);

        state.paused = false;
        state.no_effects_on = None;
        state.history_index = 0;
        state.last_change_unix = 0;

        assert_eq!(preview_fingerprint(&state), fingerprint);
    }

    #[test]
    fn preview_prewarm_uses_resolved_cache_dir_from_loaded_context() {
        let ctx = WallsCtx {
            paths: WallsPaths {
                config_dir: PathBuf::from("/tmp/config"),
                config_file: PathBuf::from("/tmp/config/config.json"),
                secrets_file: PathBuf::from("/tmp/config/secrets.json"),
                state_file: PathBuf::from("/tmp/state/state.json"),
                event_journal_file: PathBuf::from("/tmp/state/events.jsonl"),
                cache_dir: PathBuf::from("/tmp/data/cache"),
                download_dir: PathBuf::from("/tmp/data/downloaded"),
                favorites_dir: PathBuf::from("/tmp/data/favorites"),
                fetched_dir: PathBuf::from("/tmp/data/fetched"),
                compose_dir: PathBuf::from("/tmp/data/wallpaper"),
            },
            config: default_config().expect("default config"),
            secrets: Secrets::default(),
            state: State::default(),
            provider_status_report: ProviderStatusReport::default(),
        };

        let paths = preview_prewarm_paths_from_ctx(ctx);

        assert_eq!(paths.state_file, PathBuf::from("/tmp/state/state.json"));
        assert_eq!(paths.cache_dir, PathBuf::from("/tmp/data/cache"));
    }
}

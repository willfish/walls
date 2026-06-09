use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;

use walls_core::paths::WallsPaths;
use walls_core::preview_cache::{
    previewable_paths_from_state, prewarm_preview_thumbnails, DEFAULT_PREVIEW_SIZE,
};
use walls_core::state::State;

pub struct PreviewPrewarmer {
    paths: WallsPaths,
    tx: SyncSender<PreviewPrewarmJob>,
    last_fingerprint: Option<u64>,
}

struct PreviewPrewarmJob {
    state: State,
}

impl PreviewPrewarmer {
    pub fn new() -> anyhow::Result<Self> {
        let paths = WallsPaths::discover()?;
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

fn preview_fingerprint(state: &State) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.cache_queue.hash(&mut hasher);
    state.history.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

use std::path::{Path, PathBuf};

use crate::apply::ApplyTrigger;
use crate::events::{append_event_best_effort, EventRecord};
use crate::pipeline;
use crate::state::CurrentWallMetadata;

use super::WallsCtx;

#[derive(Debug, Clone)]
pub(crate) struct ApplyRequest {
    pub original: PathBuf,
    pub trigger: ApplyTrigger,
    pub wallhaven_id: Option<String>,
    pub metadata: CurrentWallMetadata,
    pub update_history: bool,
}

impl ApplyRequest {
    pub(crate) fn new(
        original: &Path,
        trigger: ApplyTrigger,
        wallhaven_id: Option<String>,
        metadata: CurrentWallMetadata,
        update_history: bool,
    ) -> Self {
        Self {
            original: original.to_path_buf(),
            trigger,
            wallhaven_id,
            metadata,
            update_history,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyResult {
    pub composed: PathBuf,
}

pub(crate) struct ApplyTransaction<'ctx> {
    ctx: &'ctx mut WallsCtx,
}

impl<'ctx> ApplyTransaction<'ctx> {
    pub(crate) fn new(ctx: &'ctx mut WallsCtx) -> Self {
        Self { ctx }
    }

    pub(crate) fn run(&mut self, request: ApplyRequest) -> anyhow::Result<ApplyResult> {
        let provider = request.metadata.provider.clone();
        let composed =
            match pipeline::compose(&self.ctx.paths, &self.ctx.config.display, &request.original) {
                Ok(composed) => composed,
                Err(error) => {
                    append_event_best_effort(
                        &self.ctx.paths.event_journal_file,
                        &EventRecord::apply_failed(
                            request.trigger,
                            &request.original,
                            None,
                            provider,
                            error.to_string(),
                        ),
                    );
                    return Err(error);
                }
            };

        if let Err(error) = crate::apply::apply_wallpaper(
            &self.ctx.config.apply,
            &composed,
            &request.original,
            self.ctx.fill_mode(),
            request.trigger,
        ) {
            append_event_best_effort(
                &self.ctx.paths.event_journal_file,
                &EventRecord::apply_failed(
                    request.trigger,
                    &request.original,
                    Some(&composed),
                    provider,
                    error.to_string(),
                ),
            );
            return Err(error);
        }

        let history_id = request.original.display().to_string();
        let source_id = request
            .original
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("local")
            .to_string();
        self.ctx.state.current = Some(crate::state::CurrentWall {
            source_id,
            wallhaven_id: request.wallhaven_id,
            provider: request.metadata.provider,
            source_url: request.metadata.source_url,
            author: request.metadata.author,
            description: request.metadata.description,
            original_path: history_id.clone(),
            composed_path: composed.display().to_string(),
            post_filter_path: Some(composed.display().to_string()),
        });
        if request.update_history {
            self.ctx.state.history.retain(|h| h != &history_id);
            self.ctx.state.history.insert(0, history_id);
            if self.ctx.state.history.len() > 1000 {
                self.ctx.state.history.truncate(1000);
            }
            self.ctx.state.history_index = 0;
        }
        self.ctx.state.last_change_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.ctx.save_state()?;
        append_event_best_effort(
            &self.ctx.paths.event_journal_file,
            &EventRecord::apply(request.trigger, &request.original, &composed, provider),
        );
        Ok(ApplyResult { composed })
    }
}

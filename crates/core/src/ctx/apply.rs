use super::{RefreshLevel, WallsCtx};
use crate::apply::{ApplyTrigger, FillMode};
use crate::error::{Result, WallsError};
use crate::pipeline;
use crate::state::CurrentWallMetadata;
use std::path::{Path, PathBuf};

impl WallsCtx {
    pub fn fill_mode(&self) -> FillMode {
        FillMode::from_display_mode(&self.config.display.mode)
    }

    pub fn apply_file(&mut self, original: &Path, trigger: ApplyTrigger) -> Result<()> {
        let original = original.to_path_buf();
        self.with_typed_state_lock(|ctx| {
            ctx.apply_file_inner(&original, trigger, None, true)
                .map_err(|source| WallsError::ApplyFile {
                    original: original.clone(),
                    source,
                })
        })
    }

    pub fn refresh_current(&mut self, level: RefreshLevel) -> Result<Option<PathBuf>> {
        self.with_typed_state_lock(|ctx| ctx.refresh_current_inner(level))
    }

    fn refresh_current_inner(&mut self, level: RefreshLevel) -> Result<Option<PathBuf>> {
        let Some(current) = self.state.current.clone() else {
            return Ok(None);
        };
        let original = PathBuf::from(&current.original_path);
        if !original.exists() {
            return Err(WallsError::CurrentOriginalMissing { path: original });
        }

        if level.recomposes_image() {
            self.apply_file_inner(
                &original,
                ApplyTrigger::Refresh,
                current.wallhaven_id.clone(),
                false,
            )
            .map_err(|source| WallsError::RefreshCurrent { source })?;
            return Ok(Some(PathBuf::from(
                self.state
                    .current
                    .as_ref()
                    .map_or(current.composed_path.as_str(), |cur| {
                        cur.composed_path.as_str()
                    }),
            )));
        }

        let composed = PathBuf::from(&current.composed_path);
        if !composed.exists() {
            return Err(WallsError::CurrentComposedMissing { path: composed });
        }
        crate::apply::apply_wallpaper(
            &self.config.apply,
            &composed,
            &original,
            self.fill_mode(),
            ApplyTrigger::Refresh,
        )
        .map_err(|source| WallsError::RefreshCurrent { source })?;
        self.state.last_change_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.save_state()
            .map_err(|source| WallsError::RefreshCurrent { source })?;
        Ok(Some(composed))
    }

    pub(crate) fn apply_file_inner(
        &mut self,
        original: &Path,
        trigger: ApplyTrigger,
        wallhaven_id: Option<String>,
        update_history: bool,
    ) -> anyhow::Result<()> {
        self.apply_file_inner_with_metadata(
            original,
            trigger,
            wallhaven_id,
            CurrentWallMetadata::default(),
            update_history,
        )
    }

    pub(crate) fn apply_file_inner_with_metadata(
        &mut self,
        original: &Path,
        trigger: ApplyTrigger,
        wallhaven_id: Option<String>,
        metadata: CurrentWallMetadata,
        update_history: bool,
    ) -> anyhow::Result<()> {
        let composed = pipeline::compose(&self.paths, &self.config.display, original)?;
        crate::apply::apply_wallpaper(
            &self.config.apply,
            &composed,
            original,
            self.fill_mode(),
            trigger,
        )?;
        let history_id = original.display().to_string();
        let source_id = original
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("local")
            .to_string();
        self.state.current = Some(crate::state::CurrentWall {
            source_id,
            wallhaven_id,
            provider: metadata.provider,
            source_url: metadata.source_url,
            author: metadata.author,
            description: metadata.description,
            original_path: history_id.clone(),
            composed_path: composed.display().to_string(),
            post_filter_path: Some(composed.display().to_string()),
        });
        if update_history {
            self.state.history.retain(|h| h != &history_id);
            self.state.history.insert(0, history_id);
            if self.state.history.len() > 1000 {
                self.state.history.truncate(1000);
            }
            self.state.history_index = 0;
        }
        self.state.last_change_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.save_state()
    }
}

use super::{RefreshLevel, WallsCtx};
use crate::apply::{ApplyTrigger, FillMode};
use crate::ctx::apply_transaction::{ApplyRequest, ApplyTransaction};
use crate::error::{Result, WallsError};
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
        ApplyTransaction::new(self).run(ApplyRequest::new(
            original,
            trigger,
            wallhaven_id,
            metadata,
            update_history,
        ))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::apply::ApplyTrigger;
    use crate::ctx::apply_transaction::ApplyRequest;
    use crate::state::CurrentWallMetadata;

    #[test]
    fn apply_request_names_transaction_inputs() {
        let metadata = CurrentWallMetadata {
            provider: Some("unsplash".into()),
            source_url: Some("https://example.test/photo".into()),
            author: Some("A. Photographer".into()),
            description: Some("trees".into()),
        };

        let request = ApplyRequest::new(
            Path::new("/tmp/original.jpg"),
            ApplyTrigger::Auto,
            Some("wallhaven-id".into()),
            metadata.clone(),
            false,
        );

        assert_eq!(request.original, Path::new("/tmp/original.jpg"));
        assert_eq!(request.trigger, ApplyTrigger::Auto);
        assert_eq!(request.wallhaven_id.as_deref(), Some("wallhaven-id"));
        assert_eq!(request.metadata.provider, metadata.provider);
        assert_eq!(request.metadata.source_url, metadata.source_url);
        assert_eq!(request.metadata.author, metadata.author);
        assert_eq!(request.metadata.description, metadata.description);
        assert!(!request.update_history);
    }
}

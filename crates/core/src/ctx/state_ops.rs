use super::WallsCtx;
use std::path::{Path, PathBuf};

impl WallsCtx {
    pub fn set_paused(&mut self, paused: bool) -> anyhow::Result<()> {
        self.with_state_lock(|ctx| {
            if ctx.state.paused != paused {
                ctx.state.paused = paused;
                ctx.save_state()?;
            }
            Ok(())
        })
    }

    pub fn toggle_pause(&mut self) -> anyhow::Result<()> {
        self.with_state_lock(|ctx| {
            ctx.state.paused = !ctx.state.paused;
            ctx.save_state()
        })
    }

    /// Move a Wallhaven id to the front of the download queue.
    pub fn prioritize_cache_id(&mut self, id: &str) -> anyhow::Result<()> {
        self.with_state_lock(|ctx| {
            ctx.state.cache_queue.retain(|q| q != id);
            ctx.state.cache_queue.insert(0, id.to_string());
            ctx.save_state()
        })
    }

    /// Path to the composed wallpaper on disk, if one is set.
    pub fn current_path(&self) -> Option<&Path> {
        self.state
            .current
            .as_ref()
            .map(|c| Path::new(&c.composed_path))
    }

    /// Metadata for the active wallpaper.
    pub fn current_meta(&self) -> Option<&crate::state::CurrentWall> {
        self.state.current.as_ref()
    }

    /// Import image files into the fetched directory (copy by default).
    pub fn fetch_files(&self, paths: &[PathBuf], move_files: bool) -> anyhow::Result<Vec<PathBuf>> {
        let mut imported = Vec::new();
        for path in paths {
            let path = crate::paths::expand_home(path);
            let dest = if move_files {
                crate::library::move_into_dir(&path, &self.paths.fetched_dir)?
            } else {
                crate::library::copy_into_dir(&path, &self.paths.fetched_dir)?
            };
            imported.push(dest);
        }
        Ok(imported)
    }

    pub fn plan_nuke_downloads(&self) -> crate::downloads::NukeDownloadsPlan {
        crate::downloads::plan_nuke_downloads(&self.paths, &self.state)
    }

    /// Clear the provider queue, or purge cached/downloaded provider files when the queue is empty.
    pub fn nuke_downloads(&mut self) -> anyhow::Result<crate::downloads::NukeDownloadsResult> {
        self.with_state_lock(|ctx| {
            let result = crate::downloads::nuke_downloads(&ctx.paths, &mut ctx.state)?;
            ctx.save_state()?;
            Ok(result)
        })
    }

    /// Delete the current wallpaper file and clear it from state/history.
    pub fn trash_current(&mut self) -> anyhow::Result<()> {
        self.with_state_lock(WallsCtx::trash_current_inner)
    }

    fn trash_current_inner(&mut self) -> anyhow::Result<()> {
        let Some(current) = self.state.current.take() else {
            anyhow::bail!("no current wallpaper");
        };
        let original = current.original_path.clone();
        let composed = current.composed_path.clone();
        if let Some(id) = current.wallhaven_id {
            self.state.cache_queue.retain(|q| q != &id);
        }
        self.state.history.retain(|h| h != &original);
        if self.state.history_index >= self.state.history.len() && !self.state.history.is_empty() {
            self.state.history_index = self.state.history.len() - 1;
        }
        Self::remove_file_if_exists(&original)?;
        if composed != original {
            Self::remove_file_if_exists(&composed)?;
        }
        self.save_state()
    }

    fn remove_file_if_exists(path: &str) -> anyhow::Result<()> {
        let p = Path::new(path);
        if p.is_file() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }

    /// Copy the current wallpaper's original file into the favorites directory.
    pub fn favorite_current(&self) -> anyhow::Result<PathBuf> {
        let current = self
            .state
            .current
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no current wallpaper"))?;
        let src = Path::new(&current.original_path);
        crate::library::copy_into_dir(src, &self.paths.favorites_dir)
    }
}

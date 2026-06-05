use super::WallsCtx;
use crate::apply::ApplyTrigger;
use crate::selection::PickInput;
use crate::state::State;
use std::path::PathBuf;

impl WallsCtx {
    pub fn collect_local_candidates(&self) -> anyhow::Result<Vec<PathBuf>> {
        use crate::sources::{enabled_sources, list_images_with_paths};
        let mut paths = Vec::new();
        for src in enabled_sources(&self.config.sources) {
            if !matches!(
                src.source_type.as_str(),
                "folder" | "favorites" | "fetched" | "image"
            ) {
                continue;
            }
            for img in
                list_images_with_paths(src, &self.paths.favorites_dir, &self.paths.fetched_dir)?
            {
                paths.push(img.path);
            }
        }
        Ok(paths)
    }

    pub async fn advance_next(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file)?;
        self.state = State::load_or_default(&self.paths.state_file)?;
        AdvanceNext::new(self).run().await
    }

    pub fn advance_prev(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.with_state_lock(WallsCtx::advance_prev_inner)
    }

    fn advance_prev_inner(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if self.state.history.len() < 2 {
            return Ok(None);
        }
        self.state.history_index = (self.state.history_index + 1).min(self.state.history.len() - 1);
        let id = self.state.history[self.state.history_index].clone();
        let path = PathBuf::from(&id);
        if path.exists() {
            self.apply_file_inner(&path, ApplyTrigger::Manual, None, false)?;
            return Ok(Some(path));
        }
        Ok(None)
    }
}

struct AdvanceNext<'ctx> {
    ctx: &'ctx mut WallsCtx,
}

impl<'ctx> AdvanceNext<'ctx> {
    fn new(ctx: &'ctx mut WallsCtx) -> Self {
        Self { ctx }
    }

    async fn run(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if self.should_skip() {
            tracing::info!("skipped: paused or change disabled");
            return Ok(None);
        }

        if let Some(path) = self.apply_cached_queue_head()? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_wallhaven_queue().await? {
            return Ok(Some(path));
        }

        self.apply_local_candidate()
    }

    fn should_skip(&self) -> bool {
        self.ctx.state.paused || !self.ctx.config.change.enabled
    }

    async fn apply_wallhaven_queue(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if !self.wallhaven_enabled() {
            return Ok(None);
        }

        let client = self.wallhaven_client()?;
        if let Some(path) = self.apply_wallhaven_queue_head(&client).await? {
            return Ok(Some(path));
        }

        crate::wallhaven::refill_wallhaven_cache(&client, &self.ctx.config, &mut self.ctx.state)
            .await?;
        self.ctx.save_state()?;
        self.apply_wallhaven_queue_head(&client).await
    }

    fn wallhaven_enabled(&self) -> bool {
        self.ctx.config.change.internet_enabled && !self.ctx.secrets.wallhaven_api_key.is_empty()
    }

    fn wallhaven_client(&self) -> anyhow::Result<crate::wallhaven::WallhavenClient> {
        crate::wallhaven::WallhavenClient::new(
            crate::wallhaven::client::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )
    }

    async fn apply_wallhaven_queue_head(
        &mut self,
        client: &crate::wallhaven::WallhavenClient,
    ) -> anyhow::Result<Option<PathBuf>> {
        let Some(id) = self.ctx.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let path = if let Some(p) = self.cached_wallhaven_path(&id) {
            p
        } else {
            let wp = client.fetch_wallpaper(&id).await?;
            client
                .download_to_cache_with_quota(
                    &wp,
                    &self.ctx.paths.cache_dir,
                    &self.ctx.paths.download_dir,
                    self.ctx.config.quota.size_mb,
                    self.ctx.config.quota.enabled,
                )
                .await?
        };
        self.ctx.state.cache_queue.remove(0);
        self.ctx
            .apply_file_inner(&path, ApplyTrigger::Auto, Some(id.clone()), true)?;
        Ok(Some(path))
    }

    fn apply_cached_queue_head(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let Some(id) = self.ctx.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let Some(path) = self.cached_wallhaven_path(&id) else {
            return Ok(None);
        };
        self.ctx.state.cache_queue.remove(0);
        self.ctx
            .apply_file_inner(&path, ApplyTrigger::Auto, Some(id), true)?;
        Ok(Some(path))
    }

    fn apply_local_candidate(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let paths = self.ctx.collect_local_candidates()?;
        if paths.is_empty() {
            tracing::info!("no wallpaper candidates");
            return Ok(None);
        }
        let ids: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let id = crate::selection::pick_next(&PickInput {
            candidates: &ids,
            recent: &self.ctx.state.history,
            avoid_recent: self.ctx.config.selection.avoid_recent,
        })?;
        let path = paths
            .into_iter()
            .find(|p| p.display().to_string() == id)
            .ok_or_else(|| anyhow::anyhow!("picked path vanished"))?;
        self.ctx
            .apply_file_inner(&path, ApplyTrigger::Auto, None, true)?;
        Ok(Some(path))
    }

    fn cached_wallhaven_path(&self, id: &str) -> Option<PathBuf> {
        crate::wallhaven::cached_wallpaper_path(&self.ctx.paths.cache_dir, id)
    }
}

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

    fn wallhaven_client(&self) -> anyhow::Result<crate::wallhaven::WallhavenClient> {
        crate::wallhaven::WallhavenClient::new(
            crate::wallhaven::client::api_base(),
            &self.secrets.wallhaven_api_key,
        )
    }

    async fn try_apply_cache_head(
        &mut self,
        client: &crate::wallhaven::WallhavenClient,
    ) -> anyhow::Result<Option<PathBuf>> {
        let Some(id) = self.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let path = if let Some(p) = self.cached_wallhaven_path(&id) {
            p
        } else {
            let wp = client.fetch_wallpaper(&id).await?;
            client
                .download_to_cache_with_quota(
                    &wp,
                    &self.paths.cache_dir,
                    &self.paths.download_dir,
                    self.config.quota.size_mb,
                    self.config.quota.enabled,
                )
                .await?
        };
        self.state.cache_queue.remove(0);
        self.apply_file_inner(&path, ApplyTrigger::Auto, Some(id.clone()), true)?;
        Ok(Some(path))
    }

    fn try_apply_cached_only(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let Some(id) = self.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let Some(path) = self.cached_wallhaven_path(&id) else {
            return Ok(None);
        };
        self.state.cache_queue.remove(0);
        self.apply_file_inner(&path, ApplyTrigger::Auto, Some(id), true)?;
        Ok(Some(path))
    }

    pub async fn advance_next(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file)?;
        self.state = State::load_or_default(&self.paths.state_file)?;
        self.advance_next_inner().await
    }

    async fn advance_next_inner(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if self.state.paused || !self.config.change.enabled {
            tracing::info!("skipped: paused or change disabled");
            return Ok(None);
        }

        if let Some(path) = self.try_apply_cached_only()? {
            return Ok(Some(path));
        }

        if self.config.change.internet_enabled && !self.secrets.wallhaven_api_key.is_empty() {
            let client = self.wallhaven_client()?;
            if let Some(path) = self.try_apply_cache_head(&client).await? {
                return Ok(Some(path));
            }
            crate::wallhaven::refill_wallhaven_cache(&client, &self.config, &mut self.state)
                .await?;
            self.save_state()?;
            if let Some(path) = self.try_apply_cache_head(&client).await? {
                return Ok(Some(path));
            }
        }

        let paths = self.collect_local_candidates()?;
        if paths.is_empty() {
            tracing::info!("no wallpaper candidates");
            return Ok(None);
        }
        let ids: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let id = crate::selection::pick_next(&PickInput {
            candidates: &ids,
            recent: &self.state.history,
            avoid_recent: self.config.selection.avoid_recent,
        })?;
        let path = paths
            .into_iter()
            .find(|p| p.display().to_string() == id)
            .ok_or_else(|| anyhow::anyhow!("picked path vanished"))?;
        self.apply_file_inner(&path, ApplyTrigger::Auto, None, true)?;
        Ok(Some(path))
    }

    fn cached_wallhaven_path(&self, id: &str) -> Option<PathBuf> {
        crate::wallhaven::cached_wallpaper_path(&self.paths.cache_dir, id)
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

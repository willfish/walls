use super::WallsCtx;
use crate::apply::ApplyTrigger;
use crate::state::State;
use rand::RngExt;
use std::collections::HashSet;
use std::path::PathBuf;

impl WallsCtx {
    pub fn collect_local_candidates(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        self.for_each_local_candidate(|path| {
            paths.push(path);
            Ok(())
        })?;
        Ok(paths)
    }

    fn for_each_local_candidate(
        &self,
        mut visit: impl FnMut(PathBuf) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        use crate::providers::{enabled_local_sources, provider_for_source};
        use crate::sources::list_images_with_paths;
        use anyhow::Context;

        for src in enabled_local_sources(&self.config.sources) {
            let provider = provider_for_source(src);
            for img in
                list_images_with_paths(src, &self.paths.favorites_dir, &self.paths.fetched_dir)
                    .with_context(|| provider.failure_scope("local source listing").to_string())?
            {
                visit(img.path)?;
            }
        }
        Ok(())
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
        use anyhow::Context;

        if !self.wallhaven_enabled() {
            return Ok(None);
        }

        let provider = crate::providers::wallhaven_provider(&self.ctx.config, &self.ctx.secrets);
        let client = self.wallhaven_client()?;
        if let Some(path) = self.apply_wallhaven_queue_head(&client, &provider).await? {
            return Ok(Some(path));
        }

        crate::wallhaven::refill_wallhaven_cache(&client, &self.ctx.config, &mut self.ctx.state)
            .await
            .with_context(|| provider.failure_scope("queue refill").to_string())?;
        self.ctx.save_state()?;
        self.apply_wallhaven_queue_head(&client, &provider).await
    }

    fn wallhaven_enabled(&self) -> bool {
        crate::providers::wallhaven_provider(&self.ctx.config, &self.ctx.secrets).enabled
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
        provider: &crate::providers::ProviderDescriptor,
    ) -> anyhow::Result<Option<PathBuf>> {
        use anyhow::Context;

        let Some(id) = self.ctx.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let path = if let Some(p) = self.cached_wallhaven_path(&id) {
            p
        } else {
            let wp = client
                .fetch_wallpaper(&id)
                .await
                .with_context(|| provider.failure_scope("metadata fetch").to_string())?;
            client
                .download_to_cache_with_quota(
                    &wp,
                    &self.ctx.paths.cache_dir,
                    &self.ctx.paths.download_dir,
                    self.ctx.config.quota.size_mb,
                    self.ctx.config.quota.enabled,
                )
                .await
                .with_context(|| provider.failure_scope("download").to_string())?
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
        let mut picker = LocalCandidatePicker::new(
            &self.ctx.state.history,
            self.ctx.config.selection.avoid_recent,
        );
        self.ctx
            .for_each_local_candidate(|path| picker.consider(path))?;
        let Some(path) = picker.finish() else {
            tracing::info!("no wallpaper candidates");
            return Ok(None);
        };
        self.ctx
            .apply_file_inner(&path, ApplyTrigger::Auto, None, true)?;
        Ok(Some(path))
    }

    fn cached_wallhaven_path(&self, id: &str) -> Option<PathBuf> {
        crate::wallhaven::cached_wallpaper_path(&self.ctx.paths.cache_dir, id)
    }
}

struct LocalCandidatePicker<'recent> {
    recent: HashSet<&'recent str>,
    seen_available: usize,
    selected_available: Option<PathBuf>,
    seen_any: usize,
    selected_any: Option<PathBuf>,
}

impl<'recent> LocalCandidatePicker<'recent> {
    fn new(recent: &'recent [String], avoid_recent: usize) -> Self {
        Self {
            recent: recent
                .iter()
                .take(avoid_recent)
                .map(String::as_str)
                .collect(),
            seen_available: 0,
            selected_available: None,
            seen_any: 0,
            selected_any: None,
        }
    }

    fn consider(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.seen_any += 1;
        if should_replace_reservoir(self.seen_any)? {
            self.selected_any = Some(path.clone());
        }

        let id = path.display().to_string();
        if self.recent.contains(id.as_str()) {
            return Ok(());
        }

        self.seen_available += 1;
        if should_replace_reservoir(self.seen_available)? {
            self.selected_available = Some(path);
        }
        Ok(())
    }

    fn finish(self) -> Option<PathBuf> {
        self.selected_available.or(self.selected_any)
    }
}

fn should_replace_reservoir(seen: usize) -> anyhow::Result<bool> {
    let upper = u64::try_from(seen)?;
    Ok(rand::rng().random_range(0..upper) == 0)
}

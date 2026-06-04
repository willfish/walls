use crate::apply::{ApplyTrigger, FillMode};
use crate::config::{load_config, load_secrets, Config, Secrets};
use crate::paths::WallsPaths;
use crate::pipeline;
use crate::selection::PickInput;
use crate::state::State;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshLevel {
    All,
    FiltersAndTexts,
    Texts,
    ClockOnly,
}

impl RefreshLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::FiltersAndTexts => "filters-and-texts",
            Self::Texts => "texts",
            Self::ClockOnly => "clock-only",
        }
    }

    fn recomposes_image(self) -> bool {
        matches!(self, Self::All | Self::FiltersAndTexts)
    }
}

impl FromStr for RefreshLevel {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "filters-and-texts" | "filters_and_texts" => Ok(Self::FiltersAndTexts),
            "texts" => Ok(Self::Texts),
            "clock-only" | "clock_only" => Ok(Self::ClockOnly),
            _ => anyhow::bail!(
                "unsupported refresh level '{value}' (expected all, filters-and-texts, texts, or clock-only)"
            ),
        }
    }
}

pub struct WallsCtx {
    pub paths: WallsPaths,
    pub config: Config,
    pub secrets: Secrets,
    pub state: State,
}

impl WallsCtx {
    pub fn load() -> anyhow::Result<Self> {
        let paths = WallsPaths::discover()?;
        Self::load_with_paths(paths)
    }

    /// Load config/state from a test or alternate root directory.
    pub fn load_from(root: &Path) -> anyhow::Result<Self> {
        let paths = WallsPaths {
            config_dir: root.to_path_buf(),
            config_file: root.join("config.json"),
            secrets_file: root.join("secrets.json"),
            state_file: root.join("state.json"),
            cache_dir: root.join("cache"),
            download_dir: root.join("downloaded"),
            favorites_dir: root.join("favorites"),
            fetched_dir: root.join("fetched"),
            compose_dir: root.join("wallpaper"),
        };
        Self::load_with_paths(paths)
    }

    pub fn load_with_paths(mut paths: WallsPaths) -> anyhow::Result<Self> {
        let config = load_config(&paths.config_file)
            .map_err(|e| anyhow::anyhow!("failed to load {}: {e}", paths.config_file.display()))?;
        let secrets = load_secrets(&paths.secrets_file)?;
        paths.apply_config_paths(&config.paths);
        paths.ensure_data_dirs()?;
        let state = State::load_or_default(&paths.state_file)?;
        Ok(Self {
            paths,
            config,
            secrets,
            state,
        })
    }

    pub fn save_state(&self) -> anyhow::Result<()> {
        self.state.save(&self.paths.state_file)
    }

    fn with_state_lock<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file)?;
        self.state = State::load_or_default(&self.paths.state_file)?;
        f(self)
    }

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

    /// Delete the current wallpaper file and clear it from state/history.
    pub fn trash_current(&mut self) -> anyhow::Result<()> {
        self.with_state_lock(|ctx| ctx.trash_current_inner())
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
        self.remove_file_if_exists(&original)?;
        if composed != original {
            self.remove_file_if_exists(&composed)?;
        }
        self.save_state()
    }

    fn remove_file_if_exists(&self, path: &str) -> anyhow::Result<()> {
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

    pub fn fill_mode(&self) -> FillMode {
        FillMode::from_display_mode(&self.config.display.mode)
    }

    pub fn apply_file(&mut self, original: &Path, trigger: ApplyTrigger) -> anyhow::Result<()> {
        let original = original.to_path_buf();
        self.with_state_lock(|ctx| ctx.apply_file_inner(&original, trigger, None, true))
    }

    pub fn refresh_current(&mut self, level: RefreshLevel) -> anyhow::Result<Option<PathBuf>> {
        self.with_state_lock(|ctx| ctx.refresh_current_inner(level))
    }

    fn refresh_current_inner(&mut self, level: RefreshLevel) -> anyhow::Result<Option<PathBuf>> {
        let Some(current) = self.state.current.clone() else {
            return Ok(None);
        };
        let original = PathBuf::from(&current.original_path);
        if !original.exists() {
            anyhow::bail!(
                "current original wallpaper does not exist: {}",
                original.display()
            );
        }

        if level.recomposes_image() {
            self.apply_file_inner(
                &original,
                ApplyTrigger::Refresh,
                current.wallhaven_id.clone(),
                false,
            )?;
            return Ok(Some(PathBuf::from(
                self.state
                    .current
                    .as_ref()
                    .map(|cur| cur.composed_path.as_str())
                    .unwrap_or(current.composed_path.as_str()),
            )));
        }

        let composed = PathBuf::from(&current.composed_path);
        if !composed.exists() {
            anyhow::bail!(
                "current composed wallpaper does not exist: {}",
                composed.display()
            );
        }
        crate::apply::apply_wallpaper(
            &self.config.apply,
            &composed,
            &original,
            self.fill_mode(),
            ApplyTrigger::Refresh,
        )?;
        self.state.last_change_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.save_state()?;
        Ok(Some(composed))
    }

    fn apply_file_inner(
        &mut self,
        original: &Path,
        trigger: ApplyTrigger,
        wallhaven_id: Option<String>,
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
            original_path: history_id.clone(),
            composed_path: composed.display().to_string(),
            post_filter_path: Some(composed.display().to_string()),
        });
        if update_history {
            if self.state.history.first().map(|s| s.as_str()) != Some(history_id.as_str()) {
                self.state.history.insert(0, history_id);
                if self.state.history.len() > 1000 {
                    self.state.history.truncate(1000);
                }
            }
            self.state.history_index = 0;
        }
        self.state.last_change_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.save_state()
    }

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
        self.with_state_lock(|ctx| ctx.advance_prev_inner())
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

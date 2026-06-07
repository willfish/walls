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

    /// Automatic rotation — respects pause and `change.enabled`.
    pub async fn advance_next(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.advance_next_mode(AdvanceMode::Auto).await
    }

    /// Explicit user action (tray, CLI, TUI) — runs even when paused or rotation is off.
    pub async fn advance_next_manual(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.advance_next_mode(AdvanceMode::Manual).await
    }

    async fn advance_next_mode(&mut self, mode: AdvanceMode) -> anyhow::Result<Option<PathBuf>> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file)?;
        self.state = State::load_or_default(&self.paths.state_file)?;
        AdvanceNext::new(self, mode).run().await
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvanceMode {
    Auto,
    Manual,
}

struct AdvanceNext<'ctx> {
    ctx: &'ctx mut WallsCtx,
    mode: AdvanceMode,
}

impl<'ctx> AdvanceNext<'ctx> {
    fn new(ctx: &'ctx mut WallsCtx, mode: AdvanceMode) -> Self {
        Self { ctx, mode }
    }

    async fn run(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if self.should_skip() {
            tracing::info!("skipped: paused or change disabled");
            return Ok(None);
        }

        if let Some(path) = self.apply_cached_queue_head()? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_unsplash_queue().await? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_wallhaven_queue().await? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_bing().await? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_json_feed().await? {
            return Ok(Some(path));
        }

        if let Some(path) = self.apply_media_rss().await? {
            return Ok(Some(path));
        }

        self.apply_local_candidate()
    }

    fn should_skip(&self) -> bool {
        if self.mode == AdvanceMode::Manual {
            return false;
        }
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
        if crate::unsplash::queue_photo_id(&id).is_some() {
            return Ok(None);
        }
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

    async fn apply_unsplash_queue(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use anyhow::Context;

        if !self.unsplash_enabled() {
            return Ok(None);
        }

        let client = self.unsplash_client()?;
        if let Some(path) = self.apply_unsplash_queue_head(&client).await? {
            return Ok(Some(path));
        }

        let provider = crate::providers::unsplash_provider(&self.ctx.config, &self.ctx.secrets);
        crate::unsplash::refill_unsplash_cache(&client, &self.ctx.config, &mut self.ctx.state)
            .await
            .with_context(|| provider.failure_scope("queue refill").to_string())?;
        self.ctx.save_state()?;
        self.apply_unsplash_queue_head(&client).await
    }

    fn unsplash_enabled(&self) -> bool {
        self.ctx.config.change.internet_enabled
            && !self.ctx.secrets.unsplash_access_key.is_empty()
            && crate::unsplash::enabled_unsplash_sources(&self.ctx.config.sources)
                .next()
                .is_some()
    }

    fn unsplash_client(&self) -> anyhow::Result<crate::unsplash::UnsplashClient> {
        crate::unsplash::UnsplashClient::new(
            crate::unsplash::client::api_base(),
            &self.ctx.secrets.unsplash_access_key,
        )
    }

    async fn apply_unsplash_queue_head(
        &mut self,
        client: &crate::unsplash::UnsplashClient,
    ) -> anyhow::Result<Option<PathBuf>> {
        use anyhow::Context;

        let Some(queue_item) = self.ctx.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        let Some(photo_id) = crate::unsplash::queue_photo_id(&queue_item) else {
            return Ok(None);
        };
        let provider = crate::providers::unsplash_provider(&self.ctx.config, &self.ctx.secrets);

        let photo = client
            .fetch_photo(photo_id)
            .await
            .with_context(|| provider.failure_scope("metadata fetch").to_string())?;
        let path = if let Some(path) =
            crate::unsplash::cached_photo_path(&self.ctx.paths.cache_dir, photo_id)
        {
            path
        } else {
            client
                .download_to_cache_with_quota(
                    &photo,
                    &self.ctx.paths.cache_dir,
                    &self.ctx.paths.download_dir,
                    self.ctx.config.quota.size_mb,
                    self.ctx.config.quota.enabled,
                )
                .await
                .with_context(|| provider.failure_scope("download").to_string())?
        };

        self.ctx.state.cache_queue.remove(0);
        let description = photo.best_description().map(str::to_string);
        self.ctx.apply_file_inner_with_metadata(
            &path,
            ApplyTrigger::Auto,
            None,
            crate::state::CurrentWallMetadata {
                provider: Some("unsplash".into()),
                source_url: Some(photo.links.html),
                author: Some(photo.user.name),
                description,
            },
            true,
        )?;
        Ok(Some(path))
    }

    fn apply_cached_queue_head(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let Some(id) = self.ctx.state.cache_queue.first().cloned() else {
            return Ok(None);
        };
        if crate::unsplash::queue_photo_id(&id).is_some() {
            return Ok(None);
        }
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

    // Minimal support for Bing provider (public endpoint, no key) so that
    // a SourceEntry {type: "bing"} can deliver a real wallpaper.
    // Fetches current daily image using reqwest (already a dep).
    async fn apply_bing(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;
        let bing_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|s| s.enabled && s.source_type == "bing")
            .collect();
        if bing_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let provider = provider_for_source(bing_sources[0]);

        let client = ::reqwest::Client::new();
        let j: serde_json::Value = client
            .get("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1")
            .send()
            .await
            .with_context(|| provider.failure_scope("bing json fetch").to_string())?
            .json()
            .await
            .with_context(|| provider.failure_scope("bing json parse").to_string())?;

        let img = &j["images"][0];
        let rel = img["url"].as_str().unwrap_or("");
        if rel.is_empty() {
            return Ok(None);
        }
        let url = format!("https://www.bing.com{rel}");

        let bytes = client
            .get(&url)
            .send()
            .await
            .with_context(|| provider.failure_scope("bing image download").to_string())?
            .bytes()
            .await
            .with_context(|| provider.failure_scope("bing bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("bing-daily.jpg");
        tokio::fs::create_dir_all(&self.ctx.paths.cache_dir)
            .await
            .ok();
        tokio::fs::write(&dest, &bytes)
            .await
            .with_context(|| provider.failure_scope("bing write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("bing-daily".into()), true)?;
        Ok(Some(dest))
    }

    // Minimal support for JSON image feed (url + optional image_path like "$.download_url").
    // Used by the default in config.example.json for the "json" provider type.
    async fn apply_json_feed(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;
        let json_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|s| s.enabled && s.source_type == "json")
            .collect();
        if json_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let src = json_sources[0];
        let provider = provider_for_source(src);
        let feed_url = src
            .url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("json source missing url"))?;

        let client = ::reqwest::Client::new();
        let j: serde_json::Value = client
            .get(feed_url)
            .send()
            .await
            .with_context(|| provider.failure_scope("json feed fetch").to_string())?
            .json()
            .await
            .with_context(|| provider.failure_scope("json feed parse").to_string())?;

        let image_url = extract_json_string(&j, src.image_path.as_deref().unwrap_or("$.url"))
            .or_else(|| {
                j.get("url")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string)
            })
            .or_else(|| {
                j.get("download_url")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string)
            })
            .ok_or_else(|| anyhow::anyhow!("no image url found in json feed"))?;

        let bytes = client
            .get(&image_url)
            .send()
            .await
            .with_context(|| provider.failure_scope("json image download").to_string())?
            .bytes()
            .await
            .with_context(|| provider.failure_scope("json bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("json-feed.jpg");
        tokio::fs::create_dir_all(&self.ctx.paths.cache_dir)
            .await
            .ok();
        tokio::fs::write(&dest, &bytes)
            .await
            .with_context(|| provider.failure_scope("json write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("json-feed".into()), true)?;
        Ok(Some(dest))
    }

    // Minimal support for Media RSS (url pointing to RSS with enclosure or media:content).
    // Supports the default in example for "mediarss" type.
    async fn apply_media_rss(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;
        let rss_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|s| s.enabled && s.source_type == "mediarss")
            .collect();
        if rss_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let src = rss_sources[0];
        let provider = provider_for_source(src);
        let rss_url = src
            .url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mediarss missing url"))?;

        let client = ::reqwest::Client::new();
        let xml = client
            .get(rss_url)
            .send()
            .await
            .with_context(|| provider.failure_scope("mediarss fetch").to_string())?
            .text()
            .await
            .with_context(|| provider.failure_scope("mediarss text").to_string())?;

        let image_url = extract_first_media_from_rss(&xml)
            .ok_or_else(|| anyhow::anyhow!("no image enclosure found in mediarss"))?;

        let bytes = client
            .get(&image_url)
            .send()
            .await
            .with_context(|| {
                provider
                    .failure_scope("mediarss image download")
                    .to_string()
            })?
            .bytes()
            .await
            .with_context(|| provider.failure_scope("mediarss bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("mediarss.jpg");
        tokio::fs::create_dir_all(&self.ctx.paths.cache_dir)
            .await
            .ok();
        tokio::fs::write(&dest, &bytes)
            .await
            .with_context(|| provider.failure_scope("mediarss write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("mediarss".into()), true)?;
        Ok(Some(dest))
    }
}

// Minimal helpers for the feed extractors (used by json/mediarss wiring).
fn extract_json_string(value: &serde_json::Value, path: &str) -> Option<String> {
    if !path.starts_with("$.") {
        return value
            .get(path)
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
    }
    let mut current = value;
    for part in path[2..].split('.') {
        let part = part.trim();
        if !part.is_empty() {
            if let Some(idx_start) = part.find('[') {
                let key = &part[..idx_start];
                if !key.is_empty() {
                    current = current.get(key)?;
                }
                // handle [0]
                if let Some(end) = part.find(']') {
                    if let Ok(i) = part[idx_start + 1..end].parse::<usize>() {
                        current = current.get(i)?;
                    }
                }
            } else {
                current = current.get(part)?;
            }
        }
    }
    current.as_str().map(ToString::to_string)
}

fn extract_first_media_from_rss(xml: &str) -> Option<String> {
    // Look for enclosure url= or media:content url= (common in image RSS)
    let re = regex::Regex::new(r#"(?:enclosure|media:content)[^>]*url=["']([^"']+)["']"#).ok()?;
    re.captures(xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
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

use super::WallsCtx;
use crate::apply::ApplyTrigger;
use crate::config::{SelectionStrategy, SourceKind};
use crate::error::{Result, WallsError};
use crate::providers::{
    ProviderAttempt, ProviderCapability, ProviderDescriptor, ProviderFailureKind, ProviderKind,
    ProviderNoCandidateReason, ProviderOperation, ProviderRetry, ProviderRunOutcome,
    ProviderStatus, ProviderStatusReport,
};
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
        self.provider_status_report = ProviderStatusReport::default();
        AdvanceNext::new(self, mode).run().await
    }

    pub fn advance_prev(&mut self) -> Result<Option<PathBuf>> {
        self.with_typed_state_lock(WallsCtx::advance_prev_inner)
    }

    fn advance_prev_inner(&mut self) -> Result<Option<PathBuf>> {
        if self.state.history.len() < 2 {
            return Ok(None);
        }
        let next_index = (self.state.history_index + 1).min(self.state.history.len() - 1);
        let id = self.state.history[next_index].clone();
        let path = PathBuf::from(&id);
        if !path.exists() {
            return Err(WallsError::PreviousOriginalMissing { path });
        }
        self.state.history_index = next_index;
        self.apply_file_inner(&path, ApplyTrigger::Manual, None, false)
            .map_err(|source| WallsError::ApplyFile {
                original: path.clone(),
                source,
            })?;
        Ok(Some(path))
    }

    pub(crate) fn record_provider_attempt(&mut self, attempt: ProviderAttempt) {
        crate::events::append_event_best_effort(
            &self.paths.event_journal_file,
            &crate::events::EventRecord::provider_attempt(attempt.clone()),
        );
        self.provider_status_report.push(attempt);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvanceMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ProviderSlot {
    Local,
    Unsplash,
    Wallhaven,
    Bing,
    Json,
    MediaRss,
    Reddit,
    Apod,
    Pixabay,
    Immich,
    Attribution,
    Spotlight,
}

struct AdvanceNext<'ctx> {
    ctx: &'ctx mut WallsCtx,
    mode: AdvanceMode,
    provider_retries: Vec<ProviderRetry>,
}

impl<'ctx> AdvanceNext<'ctx> {
    fn new(ctx: &'ctx mut WallsCtx, mode: AdvanceMode) -> Self {
        Self {
            ctx,
            mode,
            provider_retries: Vec::new(),
        }
    }

    async fn run(&mut self) -> anyhow::Result<Option<PathBuf>> {
        if self.should_skip() {
            tracing::info!("skipped: paused or change disabled");
            self.record_all_skipped(ProviderNoCandidateReason::Disabled);
            return Ok(None);
        }

        for slot in self.provider_order()? {
            if let Some(path) = self.apply_provider_slot(slot).await? {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    fn should_skip(&self) -> bool {
        if self.mode == AdvanceMode::Manual {
            return false;
        }
        self.ctx.state.paused || !self.ctx.config.change.enabled
    }

    fn provider_order(&self) -> anyhow::Result<Vec<ProviderSlot>> {
        let mut slots = Vec::new();
        for source in self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| source.enabled)
        {
            let Some(slot) = provider_slot_for_source_kind(SourceKind::parse(&source.source_type))
            else {
                continue;
            };
            if !slots.contains(&slot) {
                slots.push(slot);
            }
        }

        if slots.len() > 1 && self.ctx.config.selection.strategy == SelectionStrategy::Random {
            let start = usize::try_from(rand::rng().random_range(0..u64::try_from(slots.len())?))?;
            slots.rotate_left(start);
        }

        Ok(slots)
    }

    async fn apply_provider_slot(&mut self, slot: ProviderSlot) -> anyhow::Result<Option<PathBuf>> {
        match slot {
            ProviderSlot::Local => self.apply_local_candidate(),
            ProviderSlot::Unsplash => super::queue_providers::apply_unsplash_queue(self.ctx).await,
            ProviderSlot::Wallhaven => {
                super::queue_providers::apply_wallhaven_queue(self.ctx).await
            }
            ProviderSlot::Bing => self.apply_bing().await,
            ProviderSlot::Json => self.apply_json_feed().await,
            ProviderSlot::MediaRss => self.apply_media_rss().await,
            ProviderSlot::Reddit => {
                self.try_inline_provider(SourceKind::Reddit, true, "next configured provider")
                    .await
            }
            ProviderSlot::Apod => {
                self.try_inline_provider(SourceKind::Apod, true, "next configured provider")
                    .await
            }
            ProviderSlot::Pixabay => {
                self.try_inline_provider(SourceKind::Pixabay, true, "next configured provider")
                    .await
            }
            ProviderSlot::Immich => {
                self.try_inline_provider(SourceKind::Immich, true, "next configured provider")
                    .await
            }
            ProviderSlot::Attribution => {
                self.try_inline_provider(SourceKind::Attribution, true, "next configured provider")
                    .await
            }
            ProviderSlot::Spotlight => {
                self.try_inline_provider(SourceKind::Spotlight, false, "next configured provider")
                    .await
            }
        }
    }

    fn apply_local_candidate(&mut self) -> anyhow::Result<Option<PathBuf>> {
        let provider = local_provider();
        let mut picker = LocalCandidatePicker::new(
            &self.ctx.state.history,
            self.ctx.config.selection.avoid_recent,
        );
        self.ctx
            .for_each_local_candidate(|path| picker.consider(path))?;
        let candidate_count = picker.seen_any;
        let Some(path) = picker.finish() else {
            tracing::info!("no wallpaper candidates");
            return Ok(
                self.finish_provider_outcome(ProviderRunOutcome::not_applied(
                    provider.no_candidates(
                        ProviderNoCandidateReason::EmptyResult,
                        Some(candidate_count),
                    ),
                )),
            );
        };
        self.ctx
            .apply_file_inner(&path, ApplyTrigger::Auto, None, true)?;
        Ok(self.finish_provider_outcome(ProviderRunOutcome::applied(
            Some(path),
            provider.applied(Some(candidate_count)),
        )))
    }

    fn record(&mut self, attempt: ProviderAttempt) {
        self.ctx.record_provider_attempt(attempt);
    }

    fn finish_provider_outcome(&mut self, outcome: ProviderRunOutcome) -> Option<PathBuf> {
        let applied_path = outcome.applied_path.clone();
        self.record(outcome.attempt);
        applied_path
    }

    fn record_all_skipped(&mut self, reason: ProviderNoCandidateReason) {
        let providers = crate::providers::configured_providers(&self.ctx.config, &self.ctx.secrets);
        for provider in providers {
            self.record(
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .skipped(reason),
            );
        }
    }

    async fn try_inline_provider(
        &mut self,
        source_kind: SourceKind,
        internet_required: bool,
        fallback_provider_id: &'static str,
    ) -> anyhow::Result<Option<PathBuf>> {
        let Some(source) = self
            .ctx
            .config
            .sources
            .iter()
            .find(|source| source.enabled && SourceKind::parse(&source.source_type) == source_kind)
            .cloned()
        else {
            return Ok(None);
        };
        let provider = crate::providers::provider_for_source(&source);
        if !source.enabled {
            self.record(
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .with_status(ProviderStatus::Disabled)
                    .skipped(ProviderNoCandidateReason::Disabled)
                    .with_fallback(fallback_provider_id),
            );
            return Ok(None);
        }
        if internet_required && !self.ctx.config.change.internet_enabled {
            self.record(
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .with_status(ProviderStatus::OfflineDisabled)
                    .skipped(ProviderNoCandidateReason::OfflineDisabled)
                    .with_fallback(fallback_provider_id),
            );
            return Ok(None);
        }

        let result = match source_kind {
            SourceKind::Reddit => crate::inline_providers::try_reddit(self.ctx).await,
            SourceKind::Apod => crate::inline_providers::try_apod(self.ctx).await,
            SourceKind::Pixabay => crate::inline_providers::try_pixabay(self.ctx).await,
            SourceKind::Immich => crate::inline_providers::try_immich(self.ctx).await,
            SourceKind::Attribution => crate::inline_providers::try_attribution(self.ctx).await,
            SourceKind::Spotlight => crate::inline_providers::try_spotlight(self.ctx).await,
            _ => Ok(None),
        };

        match result {
            Ok(Some(path)) => Ok(self.finish_provider_outcome(ProviderRunOutcome::applied(
                Some(path),
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .applied(Some(1)),
            ))),
            Ok(None) => Ok(
                self.finish_provider_outcome(ProviderRunOutcome::not_applied(
                    provider
                        .attempt(ProviderOperation::AdvanceNext)
                        .no_candidates(ProviderNoCandidateReason::EmptyResult, Some(0))
                        .with_fallback(fallback_provider_id),
                )),
            ),
            Err(error) => {
                tracing::warn!(
                    provider = provider.id,
                    error = %error,
                    "provider failed, trying next source"
                );
                let (kind, status_code) = ProviderFailureKind::classify(&error);
                Ok(
                    self.finish_provider_outcome(ProviderRunOutcome::not_applied(
                        provider
                            .attempt(ProviderOperation::AdvanceNext)
                            .failed(kind, status_code, Some(error.to_string()))
                            .with_fallback(fallback_provider_id),
                    )),
                )
            }
        }
    }

    async fn try_direct_provider(
        &mut self,
        source_kind: SourceKind,
        fallback_provider_id: &'static str,
    ) -> anyhow::Result<Option<PathBuf>> {
        let Some(source) = self
            .ctx
            .config
            .sources
            .iter()
            .find(|source| source.enabled && SourceKind::parse(&source.source_type) == source_kind)
            .cloned()
        else {
            return Ok(None);
        };
        let provider = crate::providers::provider_for_source(&source);
        if !source.enabled {
            self.record(
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .with_status(ProviderStatus::Disabled)
                    .skipped(ProviderNoCandidateReason::Disabled)
                    .with_fallback(fallback_provider_id),
            );
            return Ok(None);
        }
        if !self.ctx.config.change.internet_enabled {
            self.record(
                provider
                    .attempt(ProviderOperation::AdvanceNext)
                    .with_status(ProviderStatus::OfflineDisabled)
                    .skipped(ProviderNoCandidateReason::OfflineDisabled)
                    .with_fallback(fallback_provider_id),
            );
            return Ok(None);
        }

        self.provider_retries.clear();
        let result = match source_kind {
            SourceKind::Bing => self.apply_bing_inner().await,
            SourceKind::Json => self.apply_json_feed_inner().await,
            SourceKind::MediaRss => self.apply_media_rss_inner().await,
            _ => Ok(None),
        };

        match result {
            Ok(Some(path)) => {
                let retries = std::mem::take(&mut self.provider_retries);
                Ok(self.finish_provider_outcome(ProviderRunOutcome::applied(
                    Some(path),
                    provider
                        .attempt(ProviderOperation::AdvanceNext)
                        .with_retries(retries)
                        .applied(Some(1)),
                )))
            }
            Ok(None) => {
                let retries = std::mem::take(&mut self.provider_retries);
                Ok(
                    self.finish_provider_outcome(ProviderRunOutcome::not_applied(
                        provider
                            .attempt(ProviderOperation::AdvanceNext)
                            .with_retries(retries)
                            .no_candidates(ProviderNoCandidateReason::EmptyResult, Some(0))
                            .with_fallback(fallback_provider_id),
                    )),
                )
            }
            Err(error) => {
                let retries = std::mem::take(&mut self.provider_retries);
                tracing::warn!(
                    provider = provider.id,
                    error = %error,
                    "provider failed, trying next source"
                );
                let (kind, status_code) = ProviderFailureKind::classify(&error);
                Ok(
                    self.finish_provider_outcome(ProviderRunOutcome::not_applied(
                        provider
                            .attempt(ProviderOperation::AdvanceNext)
                            .with_retries(retries)
                            .failed(kind, status_code, Some(error.to_string()))
                            .with_fallback(fallback_provider_id),
                    )),
                )
            }
        }
    }

    // Minimal support for Bing provider (public endpoint, no key) so that
    // a SourceEntry {type: "bing"} can deliver a real wallpaper.
    // Fetches current daily image using reqwest (already a dep).
    async fn apply_bing(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.try_direct_provider(SourceKind::Bing, "json").await
    }

    async fn apply_bing_inner(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::config::SourceKind;
        let bing_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| {
                source.enabled && SourceKind::parse(&source.source_type) == SourceKind::Bing
            })
            .cloned()
            .collect();
        if bing_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let mut last_error = None;
        for source in bing_sources {
            match self.apply_bing_source(source).await {
                Ok(Some(path)) => return Ok(Some(path)),
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(error) = last_error {
            return Err(error);
        }
        Ok(None)
    }

    async fn apply_bing_source(
        &mut self,
        src: crate::config::SourceEntry,
    ) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;

        let provider = provider_for_source(&src);

        let base = bing_api_base();
        let client = crate::provider_http::client()?;
        let archive_url = format!("{base}/HPImageArchive.aspx?format=js&idx=0&n=1");
        let outcome = crate::provider_http::send_with_retry_report(|| client.get(&archive_url))
            .await
            .with_context(|| provider.failure_scope("bing json fetch").to_string())?;
        self.provider_retries.extend(outcome.retries);
        let j: serde_json::Value = outcome
            .response
            .json()
            .await
            .with_context(|| provider.failure_scope("bing json parse").to_string())?;

        let img = &j["images"][0];
        let rel = img["url"].as_str().unwrap_or("");
        if rel.is_empty() {
            return Ok(None);
        }
        let url = if rel.starts_with("http://") || rel.starts_with("https://") {
            rel.to_string()
        } else {
            format!("{base}{rel}")
        };

        let outcome = crate::provider_http::send_with_retry_report(|| client.get(&url))
            .await
            .with_context(|| provider.failure_scope("bing image download").to_string())?;
        self.provider_retries.extend(outcome.retries);
        let bytes = crate::downloads::response_bytes_limited(
            outcome.response,
            crate::downloads::DEFAULT_MAX_PROVIDER_DOWNLOAD_BYTES,
            &provider.id,
        )
        .await
        .with_context(|| provider.failure_scope("bing bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("bing-daily.jpg");
        crate::downloads::write_provider_cache_with_quota(
            &dest,
            &self.ctx.paths.download_dir,
            &bytes,
            self.ctx.config.quota.size_mb,
            self.ctx.config.quota.enabled,
        )
        .await
        .with_context(|| provider.failure_scope("bing write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("bing-daily".into()), true)?;
        Ok(Some(dest))
    }

    // Minimal support for JSON image feed (url + optional image_path like "$.download_url").
    async fn apply_json_feed(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.try_direct_provider(SourceKind::Json, "mediarss").await
    }

    async fn apply_json_feed_inner(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::config::SourceKind;
        let json_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| {
                source.enabled && SourceKind::parse(&source.source_type) == SourceKind::Json
            })
            .cloned()
            .collect();
        if json_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let mut last_error = None;
        for source in json_sources {
            match self.apply_json_feed_source(source).await {
                Ok(Some(path)) => return Ok(Some(path)),
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(error) = last_error {
            return Err(error);
        }
        Ok(None)
    }

    async fn apply_json_feed_source(
        &mut self,
        src: crate::config::SourceEntry,
    ) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;

        let provider = provider_for_source(&src);
        let feed_url = src
            .url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("json source missing url"))?;

        let client = crate::provider_http::client()?;
        let outcome = crate::provider_http::send_with_retry_report(|| client.get(feed_url))
            .await
            .with_context(|| provider.failure_scope("json feed fetch").to_string())?;
        self.provider_retries.extend(outcome.retries);
        let j: serde_json::Value = outcome
            .response
            .json()
            .await
            .with_context(|| provider.failure_scope("json feed parse").to_string())?;

        let image_url =
            crate::feeds::extract_json_string(&j, src.image_path.as_deref().unwrap_or("$.url"))
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

        let outcome = crate::provider_http::send_with_retry_report(|| client.get(&image_url))
            .await
            .with_context(|| provider.failure_scope("json image download").to_string())?;
        self.provider_retries.extend(outcome.retries);
        let bytes = crate::downloads::response_bytes_limited(
            outcome.response,
            crate::downloads::DEFAULT_MAX_PROVIDER_DOWNLOAD_BYTES,
            &provider.id,
        )
        .await
        .with_context(|| provider.failure_scope("json bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("json-feed.jpg");
        crate::downloads::write_provider_cache_with_quota(
            &dest,
            &self.ctx.paths.download_dir,
            &bytes,
            self.ctx.config.quota.size_mb,
            self.ctx.config.quota.enabled,
        )
        .await
        .with_context(|| provider.failure_scope("json write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("json-feed".into()), true)?;
        Ok(Some(dest))
    }

    // Minimal support for Media RSS (url pointing to RSS with enclosure or media:content).
    // Supports the default in example for "mediarss" type.
    async fn apply_media_rss(&mut self) -> anyhow::Result<Option<PathBuf>> {
        self.try_direct_provider(SourceKind::MediaRss, "reddit")
            .await
    }

    async fn apply_media_rss_inner(&mut self) -> anyhow::Result<Option<PathBuf>> {
        use crate::config::SourceKind;
        let rss_sources: Vec<_> = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| {
                source.enabled && SourceKind::parse(&source.source_type) == SourceKind::MediaRss
            })
            .cloned()
            .collect();
        if rss_sources.is_empty() || !self.ctx.config.change.internet_enabled {
            return Ok(None);
        }
        let mut last_error = None;
        for source in rss_sources {
            match self.apply_media_rss_source(source).await {
                Ok(Some(path)) => return Ok(Some(path)),
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(error) = last_error {
            return Err(error);
        }
        Ok(None)
    }

    async fn apply_media_rss_source(
        &mut self,
        src: crate::config::SourceEntry,
    ) -> anyhow::Result<Option<PathBuf>> {
        use crate::providers::provider_for_source;
        use anyhow::Context;

        let provider = provider_for_source(&src);
        let rss_url = src
            .url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mediarss missing url"))?;

        let client = crate::provider_http::client()?;
        let outcome = crate::provider_http::send_with_retry_report(|| client.get(rss_url))
            .await
            .with_context(|| provider.failure_scope("mediarss fetch").to_string())?;
        self.provider_retries.extend(outcome.retries);
        let xml = outcome
            .response
            .text()
            .await
            .with_context(|| provider.failure_scope("mediarss text").to_string())?;

        let image_url = crate::feeds::extract_first_media_from_rss(&xml)
            .ok_or_else(|| anyhow::anyhow!("no image enclosure found in mediarss"))?;

        let outcome = crate::provider_http::send_with_retry_report(|| client.get(&image_url))
            .await
            .with_context(|| {
                provider
                    .failure_scope("mediarss image download")
                    .to_string()
            })?;
        self.provider_retries.extend(outcome.retries);
        let bytes = crate::downloads::response_bytes_limited(
            outcome.response,
            crate::downloads::DEFAULT_MAX_PROVIDER_DOWNLOAD_BYTES,
            &provider.id,
        )
        .await
        .with_context(|| provider.failure_scope("mediarss bytes").to_string())?;

        let dest = self.ctx.paths.cache_dir.join("mediarss.jpg");
        crate::downloads::write_provider_cache_with_quota(
            &dest,
            &self.ctx.paths.download_dir,
            &bytes,
            self.ctx.config.quota.size_mb,
            self.ctx.config.quota.enabled,
        )
        .await
        .with_context(|| provider.failure_scope("mediarss write cache").to_string())?;

        self.ctx
            .apply_file_inner(&dest, ApplyTrigger::Auto, Some("mediarss".into()), true)?;
        Ok(Some(dest))
    }
}

fn bing_api_base() -> String {
    std::env::var("BING_API_BASE")
        .unwrap_or_else(|_| "https://www.bing.com".to_string())
        .trim_end_matches('/')
        .to_string()
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

fn provider_slot_for_source_kind(kind: SourceKind) -> Option<ProviderSlot> {
    match kind {
        SourceKind::Folder | SourceKind::Favorites | SourceKind::Fetched | SourceKind::Image => {
            Some(ProviderSlot::Local)
        }
        SourceKind::Unsplash => Some(ProviderSlot::Unsplash),
        SourceKind::Wallhaven => Some(ProviderSlot::Wallhaven),
        SourceKind::Bing => Some(ProviderSlot::Bing),
        SourceKind::Json => Some(ProviderSlot::Json),
        SourceKind::MediaRss => Some(ProviderSlot::MediaRss),
        SourceKind::Reddit => Some(ProviderSlot::Reddit),
        SourceKind::Apod => Some(ProviderSlot::Apod),
        SourceKind::Pixabay => Some(ProviderSlot::Pixabay),
        SourceKind::Immich => Some(ProviderSlot::Immich),
        SourceKind::Attribution => Some(ProviderSlot::Attribution),
        SourceKind::Spotlight => Some(ProviderSlot::Spotlight),
        SourceKind::Weighting | SourceKind::Unknown => None,
    }
}

fn should_replace_reservoir(seen: usize) -> anyhow::Result<bool> {
    let upper = u64::try_from(seen)?;
    Ok(rand::rng().random_range(0..upper) == 0)
}

fn local_provider() -> ProviderAttempt {
    ProviderDescriptor {
        id: "local".into(),
        kind: ProviderKind::Local,
        enabled: true,
        capabilities: vec![ProviderCapability::ConfigValidation],
    }
    .attempt(ProviderOperation::LocalSourceListing)
}

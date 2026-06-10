mod commands;
mod config_block_edit;
mod config_summary;
mod edit_fields;
mod footer;
mod open_targets;
mod source_edit;
mod wallhaven_edit;

use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;

pub use config_summary::LocalSourceSummary;

use config_summary::{is_local_source, summarize_config_warnings, summarize_local_source};
pub(crate) use edit_fields::{
    block_field_kind, block_field_label, source_field_kind_for, source_field_label, EditFieldKind,
    APPLY_DISPLAY_BLOCK_FIELDS, CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY,
    CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_SOURCES, CONFIG_BLOCK_TUI, LIBRARY_BLOCK_FIELDS,
    ROTATION_BLOCK_FIELDS, SEARCH_FILTER_FIELDS, TUI_BLOCK_FIELDS, WALLHAVEN_BLOCK_FIELDS,
    WALLHAVEN_FIELDS_BLOCK,
};
use edit_fields::{
    block_field_value_at, choice_display_value, commit_block_field_buffer, cycle_choice_value,
    default_wallhaven_source_entry, reddit_time_field_locked, search_filter_field_value_at,
    source_entry_display_name, source_removal_protected, toggle_bool_value,
};
#[cfg(test)]
pub(crate) use edit_fields::{APPLY_BACKEND_CHOICES, DISPLAY_MODE_CHOICES};
use walls_core::config::{
    default_wallhaven_source, normalize_source_entry, persist_config, Config, SelectionStrategy,
    SourceEntry, WallhavenSearch,
};
use walls_core::validate::{
    validate_config_diagnostics, validate_source_edit, validate_wallhaven_edit,
};
use walls_core::WallsCtx;

use super::history_browse_view;
use super::logs_view;
use super::search_view;
use super::style::{ColorMode, StatusKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Config,
    Now,
    History,
    Browse,
    Search,
    Logs,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Config => 0,
            Tab::Now => 1,
            Tab::History => 2,
            Tab::Browse => 3,
            Tab::Search => 4,
            Tab::Logs => 5,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Config => "Config",
            Tab::Now => "Now",
            Tab::History => "History",
            Tab::Browse => "Browse",
            Tab::Search => "Search",
            Tab::Logs => "Logs",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Config,
            1 => Tab::Now,
            2 => Tab::History,
            3 => Tab::Browse,
            4 => Tab::Search,
            5 => Tab::Logs,
            _ => Tab::Config,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
    SearchInput,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditTarget {
    Block(usize),
    Source(usize),
    Wallhaven,
    SearchFilters,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EditSession {
    pub target: EditTarget,
    pub draft_source: Option<SourceEntry>,
    pub draft_block_values: std::collections::HashMap<String, String>,
    pub field_cursor: usize,
    pub field_buffer: String,
    pub validation_errors: Vec<String>,
}

pub struct SearchHit {
    pub id: String,
    pub label: String,
}

pub struct App {
    pub ctx: WallsCtx,
    pub tab: Tab,
    pub config_cursor: usize,
    pub cursor: usize,
    pub logs_cursor: usize,
    logs_seen_len: usize,
    pub config_in_subnav: bool,
    pub config_sub_cursor: usize,
    pub message: String,
    pub message_kind: StatusKind,
    pub input_mode: InputMode,
    #[allow(dead_code)]
    pub editing: Option<EditSession>,
    pub cmd_line: String,
    pub search_query: String,
    pub search_filters: WallhavenSearch,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
    pub(crate) local_source_summaries: Vec<LocalSourceSummary>,
    pub(crate) config_warnings: Vec<String>,
    pub color_mode: ColorMode,
    pub pending_nuke_confirm: bool,
    pub pending_trash_confirm: bool,
    pub show_key_help: bool,
    pub vim_pending_g: bool,
}

impl App {
    const NORMAL_TAB_NAV_HINT: &'static str = "1-6/←/→ tabs";

    pub fn new(ctx: WallsCtx) -> anyhow::Result<Self> {
        let search_filters = wallhaven_edit::first_search(&ctx.config);
        let search_query = search_filters.q.clone();
        let config_warnings = summarize_config_warnings(&ctx);
        let mut app = Self {
            ctx,
            tab: Tab::Config,
            config_cursor: 0,
            cursor: 0,
            logs_cursor: 0,
            logs_seen_len: super::log_len(),
            config_in_subnav: false,
            config_sub_cursor: 0,
            message: String::new(),
            message_kind: StatusKind::Neutral,
            input_mode: InputMode::Normal,
            editing: None,
            cmd_line: String::new(),
            search_query,
            search_filters,
            search_results: Vec::new(),
            local_candidates: Vec::new(),
            local_source_summaries: Vec::new(),
            config_warnings,
            color_mode: ColorMode::from_env(),
            pending_nuke_confirm: false,
            pending_trash_confirm: false,
            show_key_help: false,
            vim_pending_g: false,
        };
        app.refresh_local_candidates()?;
        Ok(app)
    }

    pub fn set_message(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.message = message.into();
        self.message_kind = kind;
    }

    pub fn clear_message(&mut self) {
        self.message.clear();
        self.message_kind = StatusKind::Neutral;
    }

    /// Reload config/state from disk. Recreates `config.json` with defaults if it was removed.
    pub fn reload_ctx(&mut self) -> anyhow::Result<()> {
        let paths = self.ctx.paths.clone();
        self.ctx = WallsCtx::load_with_paths(paths)?;
        self.refresh_local_candidates()?;
        Ok(())
    }

    fn refresh_local_candidates(&mut self) -> anyhow::Result<()> {
        self.local_candidates = self.ctx.collect_local_candidates().unwrap_or_default();
        self.local_source_summaries = self
            .ctx
            .config
            .sources
            .iter()
            .filter(|source| is_local_source(source))
            .map(|source| summarize_local_source(&self.ctx, source))
            .collect();
        self.config_warnings = summarize_config_warnings(&self.ctx);
        Ok(())
    }

    pub fn sources_subnav_len(&self) -> usize {
        self.ctx.config.sources.len()
    }

    pub fn move_down(&mut self) {
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            let len = self.sources_subnav_len();
            if len > 0 {
                self.config_sub_cursor = (self.config_sub_cursor + 1).min(len - 1);
            }
            return;
        }
        self.sync_log_cursor();
        let len = self.list_len();
        if len > 0 {
            let is_config = self.tab == Tab::Config;
            let old_block = if is_config {
                Some(self.config_cursor)
            } else {
                None
            };
            let cursor = self.active_cursor_mut();
            *cursor = (*cursor + 1).min(len - 1);
            if let Some(old_b) = old_block {
                let new_b = *cursor;
                if new_b != old_b && !self.is_sources_list_block(new_b) {
                    self.config_in_subnav = false;
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        self.sync_log_cursor();
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor = self.config_sub_cursor.saturating_sub(1);
            return;
        }
        let is_config = self.tab == Tab::Config;
        let old_block = if is_config {
            Some(self.config_cursor)
        } else {
            None
        };
        let cursor = self.active_cursor_mut();
        *cursor = (*cursor).saturating_sub(1);
        if let Some(old_b) = old_block {
            let new_b = *cursor;
            if new_b != old_b && !self.is_sources_list_block(new_b) {
                self.config_in_subnav = false;
            }
        }
    }

    pub fn move_first(&mut self) {
        self.move_to_row(0);
    }

    pub fn move_last(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.move_to_row(len - 1);
        }
    }

    pub fn page_up(&mut self) {
        let cursor = self.active_cursor_value();
        self.move_to_row(cursor.saturating_sub(5));
    }

    pub fn page_down(&mut self) {
        let len = self.list_len();
        if len > 0 {
            let cursor = self.active_cursor_value();
            self.move_to_row((cursor + 5).min(len - 1));
        }
    }

    fn active_cursor_value(&self) -> usize {
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor
        } else if self.tab == Tab::Config {
            self.config_cursor
        } else if self.tab == Tab::Logs {
            self.logs_cursor
        } else {
            self.cursor
        }
    }

    fn move_to_row(&mut self, row: usize) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let row = row.min(len - 1);
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            self.config_sub_cursor = row;
            return;
        }
        let was_sources = self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor);
        *self.active_cursor_mut() = row;
        if self.tab == Tab::Config && was_sources && !self.is_sources_list_block(row) {
            self.config_in_subnav = false;
        }
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.cursor = 0;
        self.config_in_subnav = false;
        self.editing = None;
        if self.tab == Tab::Logs {
            self.sync_log_cursor();
        }
    }

    pub fn sync_log_cursor(&mut self) {
        if self.tab != Tab::Logs {
            return;
        }
        let len = super::log_len();
        if len == 0 {
            self.logs_cursor = 0;
            self.logs_seen_len = 0;
            return;
        }
        if self.logs_cursor == 0 {
            self.logs_seen_len = len;
            return;
        }
        if len > self.logs_seen_len {
            self.logs_cursor = (self.logs_cursor + (len - self.logs_seen_len)).min(len - 1);
        } else if self.logs_cursor >= len {
            self.logs_cursor = len - 1;
        }
        self.logs_seen_len = len;
    }

    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Config => {
                if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
                    self.sources_subnav_len()
                } else {
                    Self::config_block_count()
                }
            }
            Tab::History => self.ctx.state.history.len(),
            Tab::Browse => self.browse_items().len(),
            Tab::Search => self.search_results.len(),
            Tab::Logs => super::log_len(),
            _ => 0,
        }
    }

    pub fn config_block_count() -> usize {
        5
    }

    fn active_cursor_mut(&mut self) -> &mut usize {
        match self.tab {
            Tab::Config => &mut self.config_cursor,
            Tab::Logs => &mut self.logs_cursor,
            _ => &mut self.cursor,
        }
    }

    pub fn history_lines(&self) -> Vec<String> {
        history_browse_view::history_lines(&self.ctx.state.history, self.cursor)
    }

    pub fn selected_history_preview_path(&self) -> Option<PathBuf> {
        history_browse_view::selected_history_preview_path(&self.ctx.state.history, self.cursor)
    }

    pub fn browse_lines(&self) -> Vec<String> {
        history_browse_view::browse_lines(self.browse_items(), self.cursor)
    }

    pub fn search_lines(&self) -> Vec<String> {
        search_view::lines(
            &self.search_query,
            &self.search_filters,
            &self.search_results,
            self.cursor,
        )
    }

    pub fn logs_lines(&self, width: u16, height: u16) -> Vec<String> {
        let logs = super::LOG_BUFFER.lock().unwrap();
        logs_view::lines(&logs, self.logs_cursor, width, height)
    }

    pub fn browse_items(&self) -> Vec<String> {
        history_browse_view::browse_items(
            &self.ctx.state.cache_queue,
            &self.local_candidates,
            &self.ctx.state.history,
        )
    }

    pub fn selected_browse_preview_path(&self) -> Option<PathBuf> {
        history_browse_view::selected_browse_preview_path(
            self.browse_items(),
            self.cursor,
            &self.ctx.paths.cache_dir,
        )
    }

    pub fn selected_search_preview_path(&self) -> Option<PathBuf> {
        let hit = self.search_results.get(self.cursor)?;
        walls_core::wallhaven::cached_wallpaper_path(&self.ctx.paths.cache_dir, &hit.id)
    }

    pub fn apply_history_selection(&mut self) -> Option<PathBuf> {
        let path = self.ctx.state.history.get(self.cursor)?.clone();
        let p = PathBuf::from(&path);
        if p.exists() {
            self.ctx.apply_file(&p, ApplyTrigger::Manual).ok()?;
            return Some(p);
        }
        None
    }

    pub async fn apply_browse_selection(&mut self) -> anyhow::Result<Option<String>> {
        let items = self.browse_items();
        let Some(line) = items.get(self.cursor) else {
            return Ok(None);
        };
        if let Some(id) = line.strip_prefix("queue: ") {
            self.ctx.prioritize_cache_id(id)?;
            if let Some(p) = self.ctx.advance_next_manual().await? {
                return Ok(Some(format!("applied queue head: {}", p.display())));
            }
            return Ok(Some("queue item not applicable".into()));
        }
        if let Some(path) = line.strip_prefix("local: ") {
            let p = PathBuf::from(path);
            if p.exists() {
                self.ctx.apply_file(&p, ApplyTrigger::Manual)?;
                return Ok(Some(format!("applied: {}", p.display())));
            }
        }
        if let Some(path) = line.strip_prefix("history: ") {
            let p = PathBuf::from(path);
            if p.exists() {
                self.ctx.apply_file(&p, ApplyTrigger::Manual)?;
                return Ok(Some(format!("applied: {}", p.display())));
            }
        }
        Ok(None)
    }

    pub async fn run_search(&mut self) -> anyhow::Result<()> {
        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let mut params = self.search_filters.clone();
        params.q = self.search_query.clone();
        let resp = client.search(&params, 1).await?;
        self.search_results = resp
            .data
            .into_iter()
            .map(|wp| SearchHit {
                id: wp.id.clone(),
                label: wp.path.clone(),
            })
            .collect();
        self.cursor = 0;
        Ok(())
    }

    pub async fn apply_search_selection(&mut self) -> anyhow::Result<Option<String>> {
        let Some(hit) = self.search_results.get(self.cursor) else {
            return Ok(None);
        };
        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let wp = client.fetch_wallpaper(&hit.id).await?;
        let path = client
            .download_to_cache_with_quota(
                &wp,
                &self.ctx.paths.cache_dir,
                &self.ctx.paths.download_dir,
                self.ctx.config.quota.size_mb,
                self.ctx.config.quota.enabled,
            )
            .await?;
        self.ctx.apply_file(&path, ApplyTrigger::Manual)?;
        Ok(Some(format!(
            "applied wallhaven-{}: {}",
            hit.id,
            path.display()
        )))
    }

    pub fn favorite_current(&mut self) -> anyhow::Result<String> {
        let dest = self.ctx.favorite_current()?;
        Ok(format!("favorited: {}", dest.display()))
    }

    pub fn trash_current(&mut self) -> anyhow::Result<String> {
        self.ctx.trash_current()?;
        Ok("trashed current wallpaper".into())
    }

    pub fn trash_current_prompt(&self) -> String {
        match self.ctx.plan_trash_current() {
            Ok(plan) => {
                let composed = plan
                    .composed_path
                    .as_ref()
                    .map(|path| format!(" + composed {path}"))
                    .unwrap_or_default();
                format!(
                    "trash: current wallpaper original {}{}? d confirm, Esc cancel",
                    plan.original_path, composed
                )
            }
            Err(e) => format!("trash: {e}"),
        }
    }

    pub fn nuke_downloads_prompt(&self) -> String {
        let plan = self.ctx.plan_nuke_downloads();
        match plan.mode {
            walls_core::downloads::NukeDownloadsMode::ClearQueue => format!(
                "provider queue: clear {} queued provider item{}? Shift+X confirm, Esc cancel",
                plan.queue_len,
                if plan.queue_len == 1 { "" } else { "s" }
            ),
            walls_core::downloads::NukeDownloadsMode::PurgeProviderFiles => format!(
                "provider files: delete {} cache + {} downloaded provider file{}? Shift+X confirm, Esc cancel",
                plan.cache_files,
                plan.download_files,
                if plan.cache_files + plan.download_files == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            walls_core::downloads::NukeDownloadsMode::ProviderReset => format!(
                "provider reset: clear {} queued, delete {} cache + {} downloaded file{}, prune {} history entr{}, current={}? Shift+X confirm, Esc cancel",
                plan.queue_len,
                plan.cache_files,
                plan.download_files,
                if plan.cache_files + plan.download_files == 1 {
                    ""
                } else {
                    "s"
                },
                plan.history_provider_entries,
                if plan.history_provider_entries == 1 { "y" } else { "ies" },
                plan.current_provider_storage
            ),
            walls_core::downloads::NukeDownloadsMode::Nothing => {
                "provider reset: no queue, cache, downloads, history, or current state to clear"
                    .into()
            }
        }
    }

    pub fn nuke_downloads(&mut self) -> anyhow::Result<String> {
        let result = self.ctx.nuke_downloads()?;
        Ok(match result.mode {
            walls_core::downloads::NukeDownloadsMode::ClearQueue => format!(
                "provider queue: cleared {} queued provider item{}",
                result.queue_cleared,
                if result.queue_cleared == 1 { "" } else { "s" }
            ),
            walls_core::downloads::NukeDownloadsMode::PurgeProviderFiles => format!(
                "provider files: removed {} cache + {} downloaded provider file{}",
                result.cache_removed,
                result.download_removed,
                if result.cache_removed + result.download_removed == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            walls_core::downloads::NukeDownloadsMode::ProviderReset => format!(
                "provider reset: cleared {} queued, removed {} cache + {} downloaded file{}, pruned {} history entr{}, current={}",
                result.queue_cleared,
                result.cache_removed,
                result.download_removed,
                if result.cache_removed + result.download_removed == 1 {
                    ""
                } else {
                    "s"
                },
                result.history_pruned,
                if result.history_pruned == 1 { "y" } else { "ies" },
                result.current_cleared
            ),
            walls_core::downloads::NukeDownloadsMode::Nothing => {
                "provider reset: no queue, cache, downloads, history, or current state to clear"
                    .into()
            }
        })
    }

    pub fn toggle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
            let idx = self.config_sub_cursor;
            if idx < config.sources.len() {
                let label = config.sources[idx]
                    .label
                    .clone()
                    .unwrap_or_else(|| config.sources[idx].source_type.clone());
                config.sources[idx].enabled = !config.sources[idx].enabled;
                persist_config(&self.ctx.paths.config_file, &config)?;
                return Ok(Some(format!(
                    "config saved: {label} enabled={}",
                    config.sources[idx].enabled
                )));
            }
        }
        let message = match self.config_cursor {
            CONFIG_BLOCK_ROTATION => {
                config.change.enabled = !config.change.enabled;
                format!("config saved: rotation enabled={}", config.change.enabled)
            }
            CONFIG_BLOCK_LIBRARY => {
                config.quota.enabled = !config.quota.enabled;
                format!("config saved: quota enabled={}", config.quota.enabled)
            }
            CONFIG_BLOCK_APPLY_DISPLAY => {
                config.display.auto_rotate = !config.display.auto_rotate;
                format!("config saved: auto rotate={}", config.display.auto_rotate)
            }
            _ => return Ok(None),
        };

        persist_config(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    pub fn cycle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        let message = match self.config_cursor {
            CONFIG_BLOCK_LIBRARY => {
                config.selection.strategy = match config.selection.strategy {
                    SelectionStrategy::Random => SelectionStrategy::Sequential,
                    SelectionStrategy::Sequential => SelectionStrategy::Random,
                };
                format!("config saved: selection={:?}", config.selection.strategy)
            }
            _ => return Ok(None),
        };

        persist_config(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    #[allow(dead_code)]
    pub fn start_edit_for_current(&mut self) {
        if self.tab != Tab::Config {
            return;
        }
        let target = if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
            self.selected_sources_subnav_edit_target()
        } else if self.is_sources_list_block(self.config_cursor) {
            match self.default_sources_edit_target() {
                Some(target) => Some(target),
                None => {
                    self.set_message(
                        StatusKind::Warning,
                        "no active sources to edit; enable or add a source first",
                    );
                    None
                }
            }
        } else {
            Some(EditTarget::Block(self.config_cursor))
        };

        let Some(target) = target else {
            return;
        };
        let Some(session) = self.edit_session_for_target(target) else {
            return;
        };
        self.editing = Some(session);
        let new_buf = self.current_edit_field_value();
        if let Some(s) = &mut self.editing {
            s.field_buffer = new_buf;
        }
        self.clear_message();
    }

    pub fn start_search_filter_edit(&mut self) {
        self.tab = Tab::Search;
        self.input_mode = InputMode::Normal;
        self.config_in_subnav = false;
        self.editing = self.edit_session_for_target(EditTarget::SearchFilters);
        let new_buf = self.current_edit_field_value();
        if let Some(s) = &mut self.editing {
            s.field_buffer = new_buf;
        }
        self.clear_message();
    }

    fn selected_sources_subnav_edit_target(&self) -> Option<EditTarget> {
        let idx = self.config_sub_cursor;
        if idx < self.ctx.config.sources.len() {
            Some(EditTarget::Source(idx))
        } else {
            None
        }
    }

    fn default_sources_edit_target(&self) -> Option<EditTarget> {
        self.ctx
            .config
            .sources
            .iter()
            .position(|source| source.enabled)
            .map(EditTarget::Source)
    }

    fn edit_session_for_target(&self, target: EditTarget) -> Option<EditSession> {
        match &target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let idx = *i;
                let mut draft = self.ctx.config.sources[idx].clone();
                normalize_source_entry(&mut draft);
                Some(EditSession {
                    target: target.clone(),
                    draft_source: Some(draft),
                    draft_block_values: std::collections::HashMap::new(),
                    field_cursor: 0,
                    field_buffer: String::new(),
                    validation_errors: vec![],
                })
            }
            EditTarget::Block(CONFIG_BLOCK_ROTATION) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: config_block_edit::rotation_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: config_block_edit::library_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: config_block_edit::display_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Block(CONFIG_BLOCK_TUI) => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: config_block_edit::tui_draft(&self.ctx.config),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::Wallhaven => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: wallhaven_edit::block_draft(
                    &self.ctx.config,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                ),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            EditTarget::SearchFilters => Some(EditSession {
                target: target.clone(),
                draft_source: None,
                draft_block_values: wallhaven_edit::search_draft(
                    &self.search_filters,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                ),
                field_cursor: 0,
                field_buffer: String::new(),
                validation_errors: vec![],
            }),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.set_message(StatusKind::Neutral, "edit cancelled");
    }

    #[allow(dead_code)]
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub(crate) fn edit_field_count(&self) -> usize {
        let Some(sess) = &self.editing else {
            return 0;
        };
        match &sess.target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let src = sess
                    .draft_source
                    .as_ref()
                    .unwrap_or(&self.ctx.config.sources[*i]);
                Self::source_editable_fields(src).len()
            }
            EditTarget::Block(CONFIG_BLOCK_ROTATION) => ROTATION_BLOCK_FIELDS.len(),
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => LIBRARY_BLOCK_FIELDS.len(),
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => APPLY_DISPLAY_BLOCK_FIELDS.len(),
            EditTarget::Block(CONFIG_BLOCK_TUI) => TUI_BLOCK_FIELDS.len(),
            EditTarget::Block(_) => 0,
            EditTarget::Wallhaven => WALLHAVEN_BLOCK_FIELDS.len(),
            EditTarget::SearchFilters => SEARCH_FILTER_FIELDS.len(),
            _ => 0,
        }
    }

    pub(crate) fn current_edit_field_kind(&self) -> EditFieldKind {
        let Some(sess) = &self.editing else {
            return EditFieldKind::Text;
        };
        match &sess.target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let src = sess
                    .draft_source
                    .as_ref()
                    .unwrap_or(&self.ctx.config.sources[*i]);
                let names = Self::source_editable_fields(src);
                if sess.field_cursor < names.len() {
                    source_field_kind_for(src, &names[sess.field_cursor])
                } else {
                    EditFieldKind::Text
                }
            }
            EditTarget::Block(block) => {
                let keys = match *block {
                    CONFIG_BLOCK_ROTATION => ROTATION_BLOCK_FIELDS,
                    CONFIG_BLOCK_LIBRARY => LIBRARY_BLOCK_FIELDS,
                    CONFIG_BLOCK_APPLY_DISPLAY => APPLY_DISPLAY_BLOCK_FIELDS,
                    CONFIG_BLOCK_TUI => TUI_BLOCK_FIELDS,
                    _ => &[] as &[&str],
                };
                if let Some(key) = keys.get(sess.field_cursor) {
                    block_field_kind(*block, key)
                } else {
                    EditFieldKind::Text
                }
            }
            EditTarget::Wallhaven => {
                if let Some(key) = WALLHAVEN_BLOCK_FIELDS.get(sess.field_cursor) {
                    block_field_kind(WALLHAVEN_FIELDS_BLOCK, key)
                } else {
                    EditFieldKind::Text
                }
            }
            EditTarget::SearchFilters => {
                if let Some(key) = SEARCH_FILTER_FIELDS.get(sess.field_cursor) {
                    block_field_kind(WALLHAVEN_FIELDS_BLOCK, key)
                } else {
                    EditFieldKind::Text
                }
            }
            _ => EditFieldKind::Text,
        }
    }

    pub(crate) fn wallhaven_block_field_locked(&self, key: &str) -> bool {
        key == "purity_nsfw" && !wallhaven_edit::api_key_present(&self.ctx.secrets)
    }

    pub(crate) fn current_edit_field_locked(&self) -> bool {
        let Some(sess) = &self.editing else {
            return false;
        };
        if matches!(&sess.target, EditTarget::Wallhaven) {
            if let Some(key) = WALLHAVEN_BLOCK_FIELDS.get(sess.field_cursor) {
                return self.wallhaven_block_field_locked(key);
            }
        }
        if matches!(&sess.target, EditTarget::SearchFilters) {
            if let Some(key) = SEARCH_FILTER_FIELDS.get(sess.field_cursor) {
                return self.wallhaven_block_field_locked(key);
            }
        }
        if let EditTarget::Source(_) = &sess.target {
            if let Some(draft) = &sess.draft_source {
                let names = Self::source_editable_fields(draft);
                if let Some(name) = names.get(sess.field_cursor) {
                    if draft.source_type == "wallhaven" && name == "purity_nsfw" {
                        return self.wallhaven_block_field_locked(name);
                    }
                    return name == "time" && reddit_time_field_locked(draft);
                }
            }
        }
        false
    }

    pub(crate) fn reddit_field_display_value(
        &self,
        src: &SourceEntry,
        key: &str,
        value: &str,
        kind: EditFieldKind,
    ) -> String {
        if key == "time" && reddit_time_field_locked(src) {
            return "n/a (top/controversial only)".into();
        }
        Self::choice_display_for_current_field(value, kind)
    }

    pub(crate) fn wallhaven_field_display_value(
        &self,
        key: &str,
        value: &str,
        kind: EditFieldKind,
    ) -> String {
        if self.wallhaven_block_field_locked(key) {
            return "unavailable (no API key)".into();
        }
        choice_display_value(kind, value)
    }

    pub(crate) fn cycle_current_edit_field(&mut self, forward: bool) {
        if self.current_edit_field_locked() {
            return;
        }
        let kind = self.current_edit_field_kind();
        let next = match kind {
            EditFieldKind::Text => return,
            EditFieldKind::Bool => {
                let current = self
                    .editing
                    .as_ref()
                    .map(|s| s.field_buffer.clone())
                    .unwrap_or_default();
                toggle_bool_value(&current)
            }
            EditFieldKind::Choice(options) => {
                let current = self
                    .editing
                    .as_ref()
                    .map(|s| s.field_buffer.clone())
                    .unwrap_or_default();
                cycle_choice_value(&current, options, forward)
            }
        };
        if let Some(sess) = &mut self.editing {
            sess.field_buffer = next;
        }
        self.commit_edit_field_buffer();
        let _ = self.save_edit_item(false);
        let refreshed = self.current_edit_field_value();
        if let Some(sess) = &mut self.editing {
            sess.field_buffer = refreshed;
        }
        self.refresh_edit_validation();
    }

    pub(crate) fn choice_display_for_current_field(value: &str, kind: EditFieldKind) -> String {
        choice_display_value(kind, value)
    }

    #[allow(dead_code)]
    pub fn source_editable_fields(src: &walls_core::config::SourceEntry) -> Vec<String> {
        source_edit::source_editable_fields(src)
    }

    #[allow(dead_code)]
    pub fn get_source_field(src: &walls_core::config::SourceEntry, name: &str) -> String {
        source_edit::get_source_field(src, name)
    }

    pub(super) fn parse_bool_like(s: &str) -> Option<bool> {
        source_edit::parse_bool_like(s)
    }

    #[allow(dead_code)]
    pub fn set_source_field(draft: &mut walls_core::config::SourceEntry, name: &str, buf: &str) {
        source_edit::set_source_field(draft, name, buf);
    }

    /// Pure value lookup for a field at a given cursor idx for a target (no reliance on live editing sess cursor).
    /// Used by up/down handlers to precompute the *new* position's buffer value without borrow conflicts.
    #[allow(dead_code)]
    pub fn edit_field_value_at(&self, target: &EditTarget, idx: usize) -> String {
        match target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                // Prefer draft for the matching editing target (so nav in edit after commits prefills
                // edited values from draft state that originated from json config).
                let src = if let Some(sess) = &self.editing {
                    if let Some(d) = &sess.draft_source {
                        if matches!(&sess.target, EditTarget::Source(j) if j == i) {
                            d
                        } else {
                            &self.ctx.config.sources[*i]
                        }
                    } else {
                        &self.ctx.config.sources[*i]
                    }
                } else {
                    &self.ctx.config.sources[*i]
                };
                let names = Self::source_editable_fields(src);
                if idx < names.len() {
                    Self::get_source_field(src, &names[idx])
                } else {
                    String::new()
                }
            }
            EditTarget::Block(block) => {
                let draft = self
                    .editing
                    .as_ref()
                    .map(|sess| sess.draft_block_values.clone())
                    .unwrap_or_default();
                block_field_value_at(&self.ctx.config, *block, &draft, idx)
            }
            EditTarget::Wallhaven => {
                let draft = self
                    .editing
                    .as_ref()
                    .map(|sess| sess.draft_block_values.clone())
                    .unwrap_or_default();
                block_field_value_at(&self.ctx.config, WALLHAVEN_FIELDS_BLOCK, &draft, idx)
            }
            EditTarget::SearchFilters => {
                let draft = self
                    .editing
                    .as_ref()
                    .map(|sess| sess.draft_block_values.clone())
                    .unwrap_or_default();
                search_filter_field_value_at(&self.search_filters, &draft, idx)
            }
            _ => String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn current_edit_field_value(&self) -> String {
        if let Some(sess) = &self.editing {
            let idx = sess.field_cursor;
            match &sess.target {
                EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                    // Prefer draft (the in-memory edit copy started from the json config at e) so that
                    // after field commits (which update only draft), moving fields prefills from the
                    // "current config item state" not stale live ctx. This makes prefill reflect the
                    // json + uncommitted edits.
                    let src = if let Some(d) = &sess.draft_source {
                        d
                    } else {
                        &self.ctx.config.sources[*i]
                    };
                    let names = Self::source_editable_fields(src);
                    if idx < names.len() {
                        return Self::get_source_field(src, &names[idx]);
                    }
                    String::new()
                }
                EditTarget::Block(block) => {
                    block_field_value_at(&self.ctx.config, *block, &sess.draft_block_values, idx)
                }
                EditTarget::Wallhaven => block_field_value_at(
                    &self.ctx.config,
                    WALLHAVEN_FIELDS_BLOCK,
                    &sess.draft_block_values,
                    idx,
                ),
                EditTarget::SearchFilters => search_filter_field_value_at(
                    &self.search_filters,
                    &sess.draft_block_values,
                    idx,
                ),
                _ => String::new(),
            }
        } else {
            String::new()
        }
    }

    pub fn is_sources_list_block(&self, block: usize) -> bool {
        block == CONFIG_BLOCK_SOURCES
    }

    fn validation_issues_for_edit(
        target: &EditTarget,
        config: &Config,
        secrets: &walls_core::config::Secrets,
        paths: &walls_core::paths::WallsPaths,
    ) -> Vec<String> {
        match target {
            EditTarget::Source(i) => validate_source_edit(*i, config, secrets, paths),
            EditTarget::Wallhaven | EditTarget::SearchFilters => {
                validate_wallhaven_edit(config, secrets)
            }
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => {
                validate_config_diagnostics(config, secrets, paths)
                    .into_iter()
                    .filter(|diagnostic| diagnostic.path.starts_with("quota."))
                    .map(|diagnostic| diagnostic.to_string())
                    .collect()
            }
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => {
                let mut issues: Vec<String> = validate_config_diagnostics(config, secrets, paths)
                    .into_iter()
                    .filter(|diagnostic| diagnostic.path.starts_with("apply."))
                    .map(|diagnostic| diagnostic.to_string())
                    .collect();
                if config.display.target_width.is_some() ^ config.display.target_height.is_some() {
                    issues.push(
                        "display target: set both target_width and target_height, or clear both"
                            .into(),
                    );
                }
                issues
            }
            EditTarget::Block(_) => Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn toggle_config_subnav(&mut self) {
        if self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor) {
            self.config_in_subnav = !self.config_in_subnav;
            if self.config_in_subnav {
                self.config_sub_cursor = 0;
            }
        }
    }

    #[allow(dead_code)]
    pub fn enter_config_subnav(&mut self) {
        if self.tab == Tab::Config && self.is_sources_list_block(self.config_cursor) {
            self.config_in_subnav = true;
            self.config_sub_cursor = 0;
        }
    }

    pub fn exit_config_subnav(&mut self) {
        self.config_in_subnav = false;
    }

    pub fn add_wallhaven_source(&mut self) -> anyhow::Result<()> {
        if self.tab != Tab::Config || !self.is_sources_list_block(self.config_cursor) {
            self.set_message(StatusKind::Warning, "add source: focus Sources first");
            return Ok(());
        }

        let mut config = self.ctx.config.clone();
        let index = config.sources.len();
        config.sources.push(default_wallhaven_source_entry());

        persist_config(&self.ctx.paths.config_file, &config)?;
        self.reload_ctx()?;
        self.tab = Tab::Config;
        self.config_cursor = CONFIG_BLOCK_SOURCES;
        self.config_in_subnav = true;
        self.config_sub_cursor = index;
        self.editing = self.edit_session_for_target(EditTarget::Source(index));
        let new_buf = self.current_edit_field_value();
        if let Some(session) = &mut self.editing {
            session.field_buffer = new_buf;
        }
        self.set_message(StatusKind::Success, "source added: Wallhaven query");
        Ok(())
    }

    pub fn remove_selected_source(&mut self) -> anyhow::Result<Option<String>> {
        if self.tab != Tab::Config || !self.is_sources_list_block(self.config_cursor) {
            self.set_message(StatusKind::Warning, "remove source: focus Sources first");
            return Ok(None);
        }
        if !self.config_in_subnav {
            self.set_message(
                StatusKind::Warning,
                "remove source: press Enter to pick a source first",
            );
            return Ok(None);
        }

        let index = self.config_sub_cursor;
        if index >= self.ctx.config.sources.len() {
            self.set_message(StatusKind::Warning, "remove source: no source selected");
            return Ok(None);
        }
        if source_removal_protected(&self.ctx.config.sources[index]) {
            self.set_message(
                StatusKind::Warning,
                "remove source: built-in library sources cannot be removed",
            );
            return Ok(None);
        }

        let mut config = self.ctx.config.clone();
        let removed = config.sources.remove(index);
        persist_config(&self.ctx.paths.config_file, &config)?;
        self.reload_ctx()?;
        self.tab = Tab::Config;
        self.config_cursor = CONFIG_BLOCK_SOURCES;
        self.config_in_subnav = true;
        self.config_sub_cursor = self.ctx.config.sources.len().saturating_sub(1).min(index);
        self.editing = None;

        Ok(Some(format!(
            "source removed: {}",
            source_entry_display_name(&removed)
        )))
    }

    pub fn can_remove_selected_source(&self) -> bool {
        if self.tab != Tab::Config
            || !self.is_sources_list_block(self.config_cursor)
            || !self.config_in_subnav
        {
            return false;
        }
        self.ctx
            .config
            .sources
            .get(self.config_sub_cursor)
            .is_some_and(|source| !source_removal_protected(source))
    }

    #[allow(dead_code)]
    pub fn refresh_edit_validation(&mut self) {
        if let Some(sess) = &mut self.editing {
            // Build a temp view of the item and validate relevant parts
            // For simplicity, clone full config, patch, run validate, filter
            let mut temp = self.ctx.config.clone();
            match &sess.target {
                EditTarget::Source(i) if *i < temp.sources.len() => {
                    if let Some(d) = &sess.draft_source {
                        temp.sources[*i] = d.clone();
                    }
                }
                EditTarget::Block(CONFIG_BLOCK_ROTATION) => {
                    config_block_edit::apply_rotation_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Block(CONFIG_BLOCK_LIBRARY) => {
                    config_block_edit::apply_library_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => {
                    config_block_edit::apply_display_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Block(CONFIG_BLOCK_TUI) => {
                    config_block_edit::apply_tui_draft(&mut temp, &sess.draft_block_values);
                }
                EditTarget::Wallhaven => {
                    wallhaven_edit::apply_block_draft(
                        &mut temp,
                        &sess.draft_block_values,
                        wallhaven_edit::api_key_present(&self.ctx.secrets),
                    );
                }
                EditTarget::SearchFilters => {}
                _ => {}
            }
            let issues = Self::validation_issues_for_edit(
                &sess.target,
                &temp,
                &self.ctx.secrets,
                &self.ctx.paths,
            );
            // keep only issues mentioning the target roughly
            sess.validation_errors = issues
                .into_iter()
                .filter(|e| match &sess.target {
                    EditTarget::Source(_) => {
                        e.contains("source")
                            || e.contains("path")
                            || e.contains("url")
                            || e.contains("key")
                            || e.contains("secrets")
                            || e.contains("unsplash")
                            || e.contains("reddit")
                    }
                    EditTarget::Wallhaven => {
                        e.contains("wallhaven") || e.contains("NSFW") || e.contains("purity")
                    }
                    _ => true,
                })
                .collect();
        }
    }

    #[allow(dead_code)]
    pub fn commit_edit_field_buffer(&mut self) {
        if let Some(sess) = &mut self.editing {
            let buf = std::mem::take(&mut sess.field_buffer);
            let field_idx = sess.field_cursor;
            match &mut sess.target {
                EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                    if let Some(draft) = &mut sess.draft_source {
                        let names = Self::source_editable_fields(draft);
                        if field_idx < names.len() {
                            let name = &names[field_idx];
                            let prev_type = draft.source_type.clone();
                            Self::set_source_field(draft, name, &buf);
                            if name == "type" && draft.source_type != prev_type {
                                normalize_source_entry(draft);
                            }
                        }
                    }
                }
                EditTarget::Block(block) => {
                    commit_block_field_buffer(
                        *block,
                        field_idx,
                        &buf,
                        &mut sess.draft_block_values,
                    );
                }
                EditTarget::Wallhaven => {
                    commit_block_field_buffer(
                        WALLHAVEN_FIELDS_BLOCK,
                        field_idx,
                        &buf,
                        &mut sess.draft_block_values,
                    );
                }
                EditTarget::SearchFilters => {
                    if let Some(key) = SEARCH_FILTER_FIELDS.get(field_idx) {
                        sess.draft_block_values
                            .insert((*key).into(), buf.trim().to_string());
                    }
                }
                _ => {}
            }
            self.refresh_edit_validation();
        }
    }

    #[allow(dead_code)]
    pub fn save_edit_item(&mut self, exit_on_success: bool) -> anyhow::Result<()> {
        // Auto-commit only if there's a pending non-empty buffer (e.g. direct Save action use, or 's' if ever mapped).
        // When caller did explicit commit first (Enter or arrow move), buffer is empty so we avoid re-committing empty
        // which would clear text fields.
        if self.editing.is_some() {
            let has_pending = if let Some(s) = &self.editing {
                !s.field_buffer.is_empty()
            } else {
                false
            };
            if has_pending {
                self.commit_edit_field_buffer();
            }
        }
        let sess = match &self.editing {
            Some(s) => s,
            None => return Ok(()),
        };
        let mut config = self.ctx.config.clone();
        let mut success_msg = "config saved via edit".to_string();
        match &sess.target {
            EditTarget::Source(i) if *i < config.sources.len() => {
                if let Some(d) = &sess.draft_source {
                    let mut saved = d.clone();
                    normalize_source_entry(&mut saved);
                    config.sources[*i] = saved;
                    success_msg = if d.source_type == "reddit" {
                        format!("config saved: reddit source #{i}")
                    } else {
                        format!("config saved: source #{} type={}", i, d.source_type)
                    };
                }
            }
            EditTarget::Block(CONFIG_BLOCK_ROTATION) => {
                config_block_edit::apply_rotation_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: rotation".into();
            }
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => {
                config_block_edit::apply_library_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: library".into();
            }
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => {
                config_block_edit::apply_display_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: display".into();
            }
            EditTarget::Block(CONFIG_BLOCK_TUI) => {
                config_block_edit::apply_tui_draft(&mut config, &sess.draft_block_values);
                success_msg = "config saved: tui preferences".into();
            }
            EditTarget::Wallhaven => {
                wallhaven_edit::apply_block_draft(
                    &mut config,
                    &sess.draft_block_values,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                );
                success_msg = "config saved: wallhaven".into();
            }
            EditTarget::SearchFilters => {
                let mut search = self.search_filters.clone();
                wallhaven_edit::apply_search_draft(
                    &mut search,
                    &sess.draft_block_values,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                );
                let mut temp = config.clone();
                let mut source = default_wallhaven_source();
                source.query = Some(search.q.clone());
                source.categories = Some(search.categories.clone());
                source.purity = Some(search.purity.clone());
                source.sorting = Some(search.sorting.clone());
                source.order = Some(search.order.clone());
                source.ratios = Some(search.ratios.clone());
                source.atleast = Some(search.atleast.clone());
                temp.sources = vec![source];
                let issues = Self::validation_issues_for_edit(
                    &sess.target,
                    &temp,
                    &self.ctx.secrets,
                    &self.ctx.paths,
                );
                if !issues.is_empty() {
                    if let Some(s) = &mut self.editing {
                        s.validation_errors = issues.clone();
                    }
                    self.set_message(
                        StatusKind::Error,
                        format!("search filters invalid: {}", issues.join("; ")),
                    );
                    return Ok(());
                }
                self.search_query = search.q.clone();
                self.search_filters = search;
                self.set_message(StatusKind::Success, "search filters updated");
                if exit_on_success {
                    self.editing = None;
                }
                return Ok(());
            }
            _ => {}
        }
        let issues = Self::validation_issues_for_edit(
            &sess.target,
            &config,
            &self.ctx.secrets,
            &self.ctx.paths,
        );
        if !issues.is_empty() {
            if let Some(s) = &mut self.editing {
                s.validation_errors = issues.clone();
            }
            self.set_message(
                StatusKind::Error,
                format!("config validation failed: {}", issues.join("; ")),
            );
            return Ok(());
        }
        persist_config(&self.ctx.paths.config_file, &config)?;
        self.set_message(StatusKind::Success, success_msg);
        // reload will happen via effect if we return it, but for simplicity here reload
        self.reload_ctx()?;
        if exit_on_success {
            self.editing = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Tab;

    #[test]
    fn tab_indices_round_trip_through_visible_order() {
        let tabs = [
            Tab::Config,
            Tab::Now,
            Tab::History,
            Tab::Browse,
            Tab::Search,
        ];

        for (index, tab) in tabs.into_iter().enumerate() {
            assert_eq!(tab.index(), index);
            assert_eq!(Tab::from_index(index), tab);
        }
    }

    #[test]
    fn unknown_tab_index_falls_back_to_config() {
        assert_eq!(Tab::from_index(usize::MAX), Tab::Config);
    }
}

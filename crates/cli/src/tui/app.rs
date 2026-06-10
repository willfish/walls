mod commands;
mod config_block_edit;
mod edit_fields;
mod edit_session;
mod footer;
mod navigation;
mod open_targets;
mod source_edit;
mod wallhaven_edit;

use std::collections::HashMap;
use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;

pub(crate) use edit_fields::{
    block_field_kind, block_field_label, source_field_kind_for, source_field_label, EditFieldKind,
    APPLY_DISPLAY_BLOCK_FIELDS, CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY,
    CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_SOURCES, CONFIG_BLOCK_TUI, LIBRARY_BLOCK_FIELDS,
    ROTATION_BLOCK_FIELDS, SEARCH_FILTER_FIELDS, TUI_BLOCK_FIELDS, WALLHAVEN_BLOCK_FIELDS,
    WALLHAVEN_FIELDS_BLOCK,
};
#[cfg(test)]
pub(crate) use edit_fields::{APPLY_BACKEND_CHOICES, DISPLAY_MODE_CHOICES};
use walls_core::config::{persist_config, SelectionStrategy, SourceEntry, WallhavenSearch};
use walls_core::validate::validate_config_diagnostics;
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
pub enum EditTarget {
    Block(usize),
    Source(usize),
    #[allow(dead_code)]
    Wallhaven,
    SearchFilters,
}

#[derive(Debug, Clone)]
pub struct EditSession {
    pub target: EditTarget,
    pub draft_source: Option<SourceEntry>,
    pub draft_block_values: HashMap<String, String>,
    pub field_cursor: usize,
    pub field_buffer: String,
    pub validation_errors: Vec<String>,
}

impl EditSession {
    pub fn new(
        target: EditTarget,
        draft_source: Option<SourceEntry>,
        draft_block_values: HashMap<String, String>,
    ) -> Self {
        Self {
            target,
            draft_source,
            draft_block_values,
            field_cursor: 0,
            field_buffer: String::new(),
            validation_errors: Vec::new(),
        }
    }
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
    pub editing: Option<EditSession>,
    pub cmd_line: String,
    pub search_query: String,
    pub search_filters: WallhavenSearch,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
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
        self.config_warnings = summarize_config_warnings(&self.ctx);
        Ok(())
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
}

fn summarize_config_warnings(ctx: &WallsCtx) -> Vec<String> {
    validate_config_diagnostics(&ctx.config, &ctx.secrets, &ctx.paths)
        .into_iter()
        .map(|diagnostic| {
            let mut warning = format!("warning: {}: {}", diagnostic.path, diagnostic.message);
            if let Some(hint) = diagnostic.hint {
                warning.push_str(&format!(" (hint: {hint})"));
            }
            warning
        })
        .collect()
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

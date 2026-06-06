use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::config::{save_config_atomic, SelectionStrategy, SourceEntry};
use walls_core::expand_home;
use walls_core::sources::list_images_with_paths;
use walls_core::validate::validate_config;
use walls_core::WallsCtx;

use super::style::ColorMode;

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

pub struct LocalSourceSummary {
    pub enabled: bool,
    pub source_type: String,
    pub label: String,
    pub path: String,
    pub status: String,
    pub candidates: usize,
}

pub struct WallhavenProviderSummary {
    pub internet_enabled: bool,
    pub api_key_present: bool,
    pub prefer: String,
    pub collections: Vec<String>,
    pub query: String,
    pub categories: String,
    pub purity: String,
    pub sorting: String,
    pub order: String,
    pub atleast: String,
    pub warnings: Vec<String>,
}

impl WallhavenProviderSummary {
    pub fn usable(&self) -> bool {
        self.internet_enabled && self.api_key_present
    }
}

pub struct App {
    pub ctx: WallsCtx,
    pub tab: Tab,
    pub config_cursor: usize,
    pub cursor: usize,
    pub config_in_subnav: bool,
    pub config_sub_cursor: usize,
    pub message: String,
    pub input_mode: InputMode,
    #[allow(dead_code)]
    pub editing: Option<EditSession>,
    pub cmd_line: String,
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
    pub(crate) local_source_summaries: Vec<LocalSourceSummary>,
    pub(crate) wallhaven_summary: WallhavenProviderSummary,
    pub(crate) config_warnings: Vec<String>,
    pub color_mode: ColorMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedCommand<'a> {
    Next,
    Prev,
    TogglePause,
    Status,
    Quit,
    Empty,
    Unknown(&'a str),
}

impl<'a> ParsedCommand<'a> {
    fn parse(line: &'a str) -> Self {
        match line.trim() {
            "next" | "n" => Self::Next,
            "prev" | "p" => Self::Prev,
            "pause" | "toggle-pause" => Self::TogglePause,
            "status" => Self::Status,
            "quit" | "q" => Self::Quit,
            "" => Self::Empty,
            other => Self::Unknown(other),
        }
    }
}

impl App {
    pub fn new(ctx: WallsCtx) -> anyhow::Result<Self> {
        let search_query = ctx.config.wallhaven.search.q.clone();
        let wallhaven_summary = summarize_wallhaven_provider(&ctx);
        let config_warnings = summarize_config_warnings(&ctx);
        let mut app = Self {
            ctx,
            tab: Tab::Config,
            config_cursor: 0,
            cursor: 0,
            config_in_subnav: false,
            config_sub_cursor: 0,
            message: String::new(),
            input_mode: InputMode::Normal,
            editing: None,
            cmd_line: String::new(),
            search_query,
            search_results: Vec::new(),
            local_candidates: Vec::new(),
            local_source_summaries: Vec::new(),
            wallhaven_summary,
            config_warnings,
            color_mode: ColorMode::from_env(),
        };
        app.refresh_local_candidates()?;
        Ok(app)
    }

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
        self.wallhaven_summary = summarize_wallhaven_provider(&self.ctx);
        self.config_warnings = summarize_config_warnings(&self.ctx);
        Ok(())
    }

    pub fn move_down(&mut self) {
        if self.tab == Tab::Config
            && self.config_in_subnav
            && self.is_sources_list_block(self.config_cursor)
        {
            let len = self.ctx.config.sources.len();
            if len > 0 {
                self.config_sub_cursor = (self.config_sub_cursor + 1).min(len - 1);
            }
            return;
        }
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

    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Config => {
                if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
                    self.ctx.config.sources.len()
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
            _ => &mut self.cursor,
        }
    }

    pub fn history_lines(&self) -> Vec<String> {
        self.ctx
            .state
            .history
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mark = if i == self.cursor { ">" } else { " " };
                format!("{mark} {h}")
            })
            .collect()
    }

    pub fn browse_lines(&self) -> Vec<String> {
        self.browse_items()
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                let mark = if i == self.cursor { ">" } else { " " };
                format!("{mark} {line}")
            })
            .collect()
    }

    pub fn search_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("query: {}", self.search_query)];
        if self.search_results.is_empty() {
            lines.push("(no results — press i to edit query, Enter to search)".into());
        } else {
            for (i, hit) in self.search_results.iter().enumerate() {
                let mark = if i == self.cursor { ">" } else { " " };
                lines.push(format!("{mark} {} — {}", hit.id, hit.label));
            }
        }
        lines
    }

    pub fn logs_lines(&self) -> Vec<String> {
        let logs = super::LOG_BUFFER.lock().unwrap();
        if logs.is_empty() {
            return vec!["(no logs captured yet)".into()];
        }
        logs.iter()
            .enumerate()
            .map(|(i, line)| {
                let mark = if i == self.cursor { ">" } else { " " };
                format!("{mark} {line}")
            })
            .collect()
    }

    pub fn browse_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        items.push("-- cache queue --".into());
        for id in &self.ctx.state.cache_queue {
            items.push(format!("queue: {id}"));
        }
        items.push("-- local folders --".into());
        for path in &self.local_candidates {
            items.push(format!("local: {}", path.display()));
        }
        items.push("-- history --".into());
        for h in &self.ctx.state.history {
            items.push(format!("history: {h}"));
        }
        items
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
            if let Some(p) = self.ctx.advance_next().await? {
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
        if self.ctx.secrets.wallhaven_api_key.is_empty() {
            anyhow::bail!("wallhaven API key missing in secrets.json");
        }
        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::client::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let mut params = self.ctx.config.wallhaven.search.clone();
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
            walls_core::wallhaven::client::api_base(),
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

    pub fn toggle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        let message = match self.config_cursor {
            0 => {
                config.change.enabled = !config.change.enabled;
                format!("config saved: rotation enabled={}", config.change.enabled)
            }
            3 => {
                config.quota.enabled = !config.quota.enabled;
                format!("config saved: quota enabled={}", config.quota.enabled)
            }
            4 => {
                config.display.auto_rotate = !config.display.auto_rotate;
                format!("config saved: auto rotate={}", config.display.auto_rotate)
            }
            _ => return Ok(None),
        };

        save_config_atomic(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    pub fn cycle_focused_config_value(&mut self) -> anyhow::Result<Option<String>> {
        let mut config = self.ctx.config.clone();
        let message = match self.config_cursor {
            3 => {
                config.selection.strategy = match config.selection.strategy {
                    SelectionStrategy::Random => SelectionStrategy::Sequential,
                    SelectionStrategy::Sequential => SelectionStrategy::Random,
                };
                format!("config saved: selection={:?}", config.selection.strategy)
            }
            _ => return Ok(None),
        };

        save_config_atomic(&self.ctx.paths.config_file, &config)?;
        Ok(Some(message))
    }

    #[allow(dead_code)]
    pub fn start_edit_for_current(&mut self) {
        if self.tab != Tab::Config {
            return;
        }
        // Support subnav for Sources block: if in subnav, target the sub item
        if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
            let idx = self.config_sub_cursor;
            if idx < self.ctx.config.sources.len() {
                let target = EditTarget::Source(idx);
                let session = EditSession {
                    target,
                    draft_source: Some(self.ctx.config.sources[idx].clone()),
                    draft_block_values: std::collections::HashMap::new(),
                    field_cursor: 0,
                    field_buffer: String::new(),
                    validation_errors: vec![],
                };
                self.editing = Some(session);
                let new_buf = self.current_edit_field_value();
                if let Some(s) = &mut self.editing {
                    s.field_buffer = new_buf;
                }
                self.message.clear();
                return;
            }
        }
        // block target (or first source fallback for old tests)
        let target = if !self.ctx.config.sources.is_empty() && self.config_cursor == 1 {
            EditTarget::Source(0)
        } else {
            EditTarget::Block(self.config_cursor)
        };
        let session = match &target {
            EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                let idx = *i;
                EditSession {
                    target: target.clone(),
                    draft_source: Some(self.ctx.config.sources[idx].clone()),
                    draft_block_values: std::collections::HashMap::new(),
                    field_cursor: 0,
                    field_buffer: String::new(),
                    validation_errors: vec![],
                }
            }
            EditTarget::Block(0) => {
                let mut vals = std::collections::HashMap::new();
                vals.insert("enabled".into(), self.ctx.config.change.enabled.to_string());
                vals.insert(
                    "interval".into(),
                    self.ctx.config.change.interval_secs.to_string(),
                );
                vals.insert(
                    "internet".into(),
                    self.ctx.config.change.internet_enabled.to_string(),
                );
                EditSession {
                    target: target.clone(),
                    draft_source: None,
                    draft_block_values: vals,
                    field_cursor: 0,
                    field_buffer: String::new(),
                    validation_errors: vec![],
                }
            }
            _ => return,
        };
        self.editing = Some(session);
        let new_buf = self.current_edit_field_value();
        if let Some(s) = &mut self.editing {
            s.field_buffer = new_buf;
        }
        self.message.clear();
    }

    #[allow(dead_code)]
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.message = "edit cancelled".into();
    }

    #[allow(dead_code)]
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Ordered list of editable field names for a given source type.
    /// This is the single source of truth for "100% necessary fields per config item".
    /// Includes only fields that are part of SourceEntry, appear in example.json or tests for the type,
    /// or are actively read by core (local path, json url+image_path, mediarss url, unsplash params, etc).
    /// Omits title_path (serde compat only, never used in logic).
    /// Omits attribution's "source"/"author" (present in some example but not modeled in SourceEntry).
    #[allow(dead_code)]
    pub fn source_editable_fields(src: &walls_core::config::SourceEntry) -> Vec<String> {
        let mut f = vec![
            "enabled".to_string(),
            "type".to_string(),
            "label".to_string(),
        ];
        let t = src.source_type.as_str();
        match t {
            "folder" | "image" | "favorites" | "fetched" => {
                f.push("path".into());
            }
            "json" => {
                f.push("url".into());
                f.push("image_path".into());
            }
            "mediarss" => {
                f.push("url".into());
            }
            "attribution" => {
                f.push("url".into());
            }
            "unsplash" => {
                f.push("url".into());
                f.push("query".into());
                f.push("collection".into());
                f.push("user".into());
                f.push("topic".into());
                f.push("orientation".into());
            }
            "reddit" | "weighting" => {
                f.push("query".into());
            }
            "pixabay" => {
                f.push("query".into());
                f.push("api_key".into());
            }
            "immich" => {
                f.push("url".into());
                f.push("api_key".into());
            }
            // bing, apod, spotlight, wallhaven (global), others: no per-source extras beyond common
            _ => {}
        }
        f
    }

    #[allow(dead_code)]
    pub fn get_source_field(src: &walls_core::config::SourceEntry, name: &str) -> String {
        match name {
            "enabled" => src.enabled.to_string(),
            "type" => src.source_type.clone(),
            "label" => src.label.clone().unwrap_or_default(),
            "url" => src.url.clone().unwrap_or_default(),
            "path" => src.path.clone().unwrap_or_default(),
            "image_path" => src.image_path.clone().unwrap_or_default(),
            "query" => src.query.clone().unwrap_or_default(),
            "api_key" => src.api_key.clone().unwrap_or_default(),
            "collection" => src.collection.clone().unwrap_or_default(),
            "user" => src.user.clone().unwrap_or_default(),
            "topic" => src.topic.clone().unwrap_or_default(),
            "orientation" => src.orientation.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Lenient bool parser for edit buffers (user may type t/f/1/0/yes/no/on/off/true/false).
    /// Prevents "I set false, s just fails" from strict parse only accepting "true"/"false".
    fn parse_bool_like(s: &str) -> Option<bool> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "true" | "t" | "1" | "yes" | "y" | "on" => Some(true),
            "false" | "f" | "0" | "no" | "n" | "off" => Some(false),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn set_source_field(draft: &mut walls_core::config::SourceEntry, name: &str, buf: &str) {
        let trimmed = buf.trim();
        let v = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        match name {
            "enabled" => {
                draft.enabled = Self::parse_bool_like(trimmed).unwrap_or(draft.enabled);
            }
            "type" if !trimmed.is_empty() => {
                draft.source_type = trimmed.to_string();
            }
            "label" => draft.label = v,
            "url" => draft.url = v,
            "path" => draft.path = v,
            "image_path" => draft.image_path = v,
            "query" => draft.query = v,
            "api_key" => draft.api_key = v,
            "collection" => draft.collection = v,
            "user" => draft.user = v,
            "topic" => draft.topic = v,
            "orientation" => draft.orientation = v,
            _ => {}
        }
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
            EditTarget::Block(0) => {
                let key = match idx {
                    0 => "enabled",
                    1 => "interval",
                    2 => "internet",
                    _ => "",
                };
                if !key.is_empty() {
                    if let Some(sess) = &self.editing {
                        if let Some(v) = sess.draft_block_values.get(key) {
                            return v.clone();
                        }
                    }
                }
                match idx {
                    0 => self.ctx.config.change.enabled.to_string(),
                    1 => self.ctx.config.change.interval_secs.to_string(),
                    2 => self.ctx.config.change.internet_enabled.to_string(),
                    _ => String::new(),
                }
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
                EditTarget::Block(0) => {
                    // For blocks, prefer draft_block_values (populated at start from json/config) so
                    // prefills after partial commits come from edit state.
                    let key = match idx {
                        0 => "enabled",
                        1 => "interval",
                        2 => "internet",
                        _ => "",
                    };
                    if !key.is_empty() {
                        if let Some(v) = sess.draft_block_values.get(key) {
                            return v.clone();
                        }
                    }
                    match idx {
                        0 => self.ctx.config.change.enabled.to_string(),
                        1 => self.ctx.config.change.interval_secs.to_string(),
                        2 => self.ctx.config.change.internet_enabled.to_string(),
                        _ => String::new(),
                    }
                }
                _ => String::new(),
            }
        } else {
            String::new()
        }
    }

    pub fn is_sources_list_block(&self, block: usize) -> bool {
        block == 1 // the "Sources" block (was Local sources)
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

    #[allow(dead_code)]
    pub fn refresh_edit_validation(&mut self) {
        if let Some(sess) = &mut self.editing {
            // Build a temp view of the item and validate relevant parts
            // For simplicity, clone full config, patch, run validate, filter
            let mut temp = self.ctx.config.clone();
            match &sess.target {
                EditTarget::Source(i) if *i < temp.sources.len() => {
                    if let Some(d) = &sess.draft_source {
                        // apply buffer if any to the draft for live
                        // (in practice commit does full, here just use current draft + buffer for current field is complex; simple: use draft)
                        temp.sources[*i] = d.clone();
                    }
                }
                _ => {}
            }
            let issues =
                walls_core::validate::validate_config(&temp, &self.ctx.secrets, &self.ctx.paths);
            // keep only issues mentioning the target roughly
            sess.validation_errors = issues
                .into_iter()
                .filter(|e| match &sess.target {
                    EditTarget::Source(_) => {
                        e.contains("source")
                            || e.contains("path")
                            || e.contains("url")
                            || e.contains("key")
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
                            Self::set_source_field(draft, name, &buf);
                        }
                    }
                }
                EditTarget::Block(0) => {
                    // rotation scalars
                    match field_idx {
                        0 => {
                            sess.draft_block_values
                                .insert("enabled".into(), buf.trim().to_string());
                        }
                        1 => {
                            sess.draft_block_values
                                .insert("interval".into(), buf.trim().to_string());
                        }
                        2 => {
                            sess.draft_block_values
                                .insert("internet".into(), buf.trim().to_string());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            self.refresh_edit_validation();
        }
    }

    #[allow(dead_code)]
    pub fn save_edit_item(&mut self) -> anyhow::Result<()> {
        // Auto-commit any pending field buffer so user can type a change then hit 's' directly
        // (without final Enter to commit the last field). This fixes the common "I edited the value
        // then s just fails" (change not in draft, or validation on old).
        if self.editing.is_some() {
            self.commit_edit_field_buffer();
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
                    config.sources[*i] = d.clone();
                    success_msg = format!("config saved: source #{} type={}", i, d.source_type);
                }
            }
            EditTarget::Block(0) => {
                // apply rotation values (simplified)
                if let Some(v) = sess.draft_block_values.get("enabled") {
                    config.change.enabled =
                        Self::parse_bool_like(v).unwrap_or(config.change.enabled);
                }
                if let Some(v) = sess.draft_block_values.get("interval") {
                    if let Ok(n) = v.parse() {
                        config.change.interval_secs = n;
                    }
                }
                if let Some(v) = sess.draft_block_values.get("internet") {
                    config.change.internet_enabled =
                        Self::parse_bool_like(v).unwrap_or(config.change.internet_enabled);
                }
                success_msg = "config saved: rotation".into();
            }
            _ => {}
        }
        // strict validate
        let issues =
            walls_core::validate::validate_config(&config, &self.ctx.secrets, &self.ctx.paths);
        if !issues.is_empty() {
            if let Some(s) = &mut self.editing {
                s.validation_errors = issues.clone();
            }
            self.message = format!("config validation failed: {}", issues.join("; "));
            return Ok(());
        }
        save_config_atomic(&self.ctx.paths.config_file, &config)?;
        self.message = success_msg;
        // reload will happen via effect if we return it, but for simplicity here reload
        self.reload_ctx()?;
        self.editing = None;
        Ok(())
    }

    pub fn run_command(&mut self, rt: &tokio::runtime::Handle) -> anyhow::Result<Option<String>> {
        let msg = match ParsedCommand::parse(&self.cmd_line) {
            ParsedCommand::Next => {
                match tokio::task::block_in_place(|| rt.block_on(self.ctx.advance_next())) {
                    Ok(Some(p)) => format!("next: {}", p.display()),
                    Ok(None) => "next: no change".into(),
                    Err(e) => format!("next error: {e}"),
                }
            }
            ParsedCommand::Prev => match self.ctx.advance_prev() {
                Ok(Some(p)) => format!("prev: {}", p.display()),
                Ok(None) => "prev: none".into(),
                Err(e) => format!("prev error: {e}"),
            },
            ParsedCommand::TogglePause => {
                self.ctx.toggle_pause()?;
                format!("paused: {}", self.ctx.state.paused)
            }
            ParsedCommand::Status => format!(
                "paused={} history={} queue={}",
                self.ctx.state.paused,
                self.ctx.state.history.len(),
                self.ctx.state.cache_queue.len()
            ),
            ParsedCommand::Quit => return Ok(None),
            ParsedCommand::Empty => "(empty command)".into(),
            ParsedCommand::Unknown(other) => {
                format!("unknown command: {other} (try :next :prev :pause :status :quit)")
            }
        };
        Ok(Some(msg))
    }

    pub fn footer_keys(&self) -> String {
        if self.is_editing() {
            return "edit: j/k fields | type/Backspace | Enter commit field | s save | Esc cancel | q".into();
        }
        let keys = match self.input_mode {
            InputMode::Command => format!(":{}_ | Enter run Esc cancel", self.cmd_line),
            InputMode::SearchInput => {
                "Search: type query | Enter search Esc cancel | i".to_string()
            }
            InputMode::Normal => match self.tab {
                Tab::Search => {
                    "5 Search | i edit query Enter search | j/k | Enter apply | : cmd".into()
                }
                Tab::Config => {
                    "1 Config | j/k | Enter (subnav for lists) | e edit | t toggle | n/p | space pause | : cmd".into()
                }
                _ => "1-6 tabs | n/p next/prev | f favorite d trash | space pause | : cmd".into(),
            },
        };
        format!("{keys} | q quit")
    }
}

fn is_local_source(source: &SourceEntry) -> bool {
    matches!(
        source.source_type.as_str(),
        "folder" | "image" | "favorites" | "fetched"
    )
}

fn summarize_local_source(ctx: &WallsCtx, source: &SourceEntry) -> LocalSourceSummary {
    let label = source
        .label
        .clone()
        .unwrap_or_else(|| source.source_type.clone());
    let path = match source.source_type.as_str() {
        "favorites" => Some(ctx.paths.favorites_dir.clone()),
        "fetched" => Some(ctx.paths.fetched_dir.clone()),
        "folder" | "image" => source.path.as_ref().map(expand_home),
        _ => None,
    };

    let Some(path) = path else {
        return LocalSourceSummary {
            enabled: source.enabled,
            source_type: source.source_type.clone(),
            label,
            path: "(not configured)".into(),
            status: "missing path".into(),
            candidates: 0,
        };
    };

    let path_status = if path.exists() {
        "ready"
    } else {
        "missing path"
    };
    let enabled_status = if source.enabled { "" } else { "disabled, " };
    let candidates =
        list_images_with_paths(source, &ctx.paths.favorites_dir, &ctx.paths.fetched_dir)
            .map_or(0, |images| images.len());

    LocalSourceSummary {
        enabled: source.enabled,
        source_type: source.source_type.clone(),
        label,
        path: path.display().to_string(),
        status: format!("{enabled_status}{path_status}"),
        candidates,
    }
}

fn summarize_wallhaven_provider(ctx: &WallsCtx) -> WallhavenProviderSummary {
    let search = &ctx.config.wallhaven.search;
    let api_key_present = !ctx.secrets.wallhaven_api_key.trim().is_empty();
    let query = if search.q.trim().is_empty() {
        "(empty query)".into()
    } else {
        search.q.clone()
    };
    let collections = ctx
        .config
        .wallhaven
        .collections
        .iter()
        .map(|collection| {
            let label = collection.label.as_deref().unwrap_or("collection");
            format!("{}: {}/{}", label, collection.username, collection.id)
        })
        .collect();

    let mut warnings = Vec::new();
    if !ctx.config.change.internet_enabled {
        warnings.push("warning: online sources disabled".into());
    }
    if !api_key_present {
        warnings.push("warning: API key missing; search and downloads are unavailable".into());
    }
    if search.purity.chars().nth(2) == Some('1') {
        warnings.push("warning: NSFW purity requires Wallhaven account access".into());
    }

    WallhavenProviderSummary {
        internet_enabled: ctx.config.change.internet_enabled,
        api_key_present,
        prefer: format!("{:?}", ctx.config.wallhaven.prefer),
        collections,
        query,
        categories: search.categories.clone(),
        purity: search.purity.clone(),
        sorting: search.sorting.clone(),
        order: search.order.clone(),
        atleast: search.atleast.clone(),
        warnings,
    }
}

fn summarize_config_warnings(ctx: &WallsCtx) -> Vec<String> {
    validate_config(&ctx.config, &ctx.secrets, &ctx.paths)
        .into_iter()
        .map(|warning| format!("warning: {warning}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ParsedCommand, Tab};

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

    #[test]
    fn command_parser_trims_and_maps_dispatch_aliases() {
        assert_eq!(ParsedCommand::parse(" next "), ParsedCommand::Next);
        assert_eq!(ParsedCommand::parse("n"), ParsedCommand::Next);
        assert_eq!(ParsedCommand::parse("prev"), ParsedCommand::Prev);
        assert_eq!(ParsedCommand::parse("p"), ParsedCommand::Prev);
        assert_eq!(ParsedCommand::parse("pause"), ParsedCommand::TogglePause);
        assert_eq!(
            ParsedCommand::parse("toggle-pause"),
            ParsedCommand::TogglePause
        );
        assert_eq!(ParsedCommand::parse("status"), ParsedCommand::Status);
        assert_eq!(ParsedCommand::parse("quit"), ParsedCommand::Quit);
        assert_eq!(ParsedCommand::parse("q"), ParsedCommand::Quit);
    }

    #[test]
    fn command_parser_distinguishes_empty_and_unknown_commands() {
        assert_eq!(ParsedCommand::parse("  "), ParsedCommand::Empty);
        assert_eq!(ParsedCommand::parse("wat"), ParsedCommand::Unknown("wat"));
    }
}

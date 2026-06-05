use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::config::SourceEntry;
use walls_core::expand_home;
use walls_core::sources::list_images_with_paths;
use walls_core::WallsCtx;

use super::style::ColorMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Config,
    Now,
    History,
    Browse,
    Search,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Config => 0,
            Tab::Now => 1,
            Tab::History => 2,
            Tab::Browse => 3,
            Tab::Search => 4,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Config => "Config",
            Tab::Now => "Now",
            Tab::History => "History",
            Tab::Browse => "Browse",
            Tab::Search => "Search",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Config,
            1 => Tab::Now,
            2 => Tab::History,
            3 => Tab::Browse,
            4 => Tab::Search,
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
    pub message: String,
    pub input_mode: InputMode,
    pub cmd_line: String,
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
    pub(crate) local_source_summaries: Vec<LocalSourceSummary>,
    pub(crate) wallhaven_summary: WallhavenProviderSummary,
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
        let mut app = Self {
            ctx,
            tab: Tab::Config,
            config_cursor: 0,
            cursor: 0,
            message: String::new(),
            input_mode: InputMode::Normal,
            cmd_line: String::new(),
            search_query,
            search_results: Vec::new(),
            local_candidates: Vec::new(),
            local_source_summaries: Vec::new(),
            wallhaven_summary,
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
        Ok(())
    }

    pub fn move_down(&mut self) {
        let len = self.list_len();
        if len > 0 {
            let cursor = self.active_cursor_mut();
            *cursor = (*cursor + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        let cursor = self.active_cursor_mut();
        *cursor = (*cursor).saturating_sub(1);
    }

    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Config => Self::config_block_count(),
            Tab::History => self.ctx.state.history.len(),
            Tab::Browse => self.browse_items().len(),
            Tab::Search => self.search_results.len(),
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

    pub fn run_command(&mut self, rt: &tokio::runtime::Handle) -> anyhow::Result<Option<String>> {
        let msg = match ParsedCommand::parse(&self.cmd_line) {
            ParsedCommand::Next => match rt.block_on(self.ctx.advance_next()) {
                Ok(Some(p)) => format!("next: {}", p.display()),
                Ok(None) => "next: no change".into(),
                Err(e) => format!("next error: {e}"),
            },
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
                    "1 Config | j/k focus block | n/p next/prev | space pause | : cmd".into()
                }
                _ => "1-5 tabs | n/p next/prev | f favorite d trash | space pause | : cmd".into(),
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

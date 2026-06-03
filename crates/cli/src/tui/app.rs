use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::WallsCtx;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Status,
    Now,
    History,
    Browse,
    Search,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Status => 0,
            Tab::Now => 1,
            Tab::History => 2,
            Tab::Browse => 3,
            Tab::Search => 4,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Status => "Status",
            Tab::Now => "Now",
            Tab::History => "History",
            Tab::Browse => "Browse",
            Tab::Search => "Search",
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Tab::Status,
            1 => Tab::Now,
            2 => Tab::History,
            3 => Tab::Browse,
            4 => Tab::Search,
            _ => Tab::Status,
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

pub struct App {
    pub ctx: WallsCtx,
    pub tab: Tab,
    pub cursor: usize,
    pub message: String,
    pub input_mode: InputMode,
    pub cmd_line: String,
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub(crate) local_candidates: Vec<PathBuf>,
}

impl App {
    pub fn new(ctx: WallsCtx) -> anyhow::Result<Self> {
        let search_query = ctx.config.wallhaven.search.q.clone();
        let mut app = Self {
            ctx,
            tab: Tab::Status,
            cursor: 0,
            message: String::new(),
            input_mode: InputMode::Normal,
            cmd_line: String::new(),
            search_query,
            search_results: Vec::new(),
            local_candidates: Vec::new(),
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
        Ok(())
    }

    pub fn move_down(&mut self) {
        let len = self.list_len();
        if len > 0 {
            self.cursor = (self.cursor + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::History => self.ctx.state.history.len(),
            Tab::Browse => self.browse_items().len(),
            Tab::Search => self.search_results.len(),
            _ => 0,
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
        let line = self.cmd_line.trim();
        let msg = match line {
            "next" | "n" => match rt.block_on(self.ctx.advance_next()) {
                Ok(Some(p)) => format!("next: {}", p.display()),
                Ok(None) => "next: no change".into(),
                Err(e) => format!("next error: {e}"),
            },
            "prev" | "p" => match self.ctx.advance_prev() {
                Ok(Some(p)) => format!("prev: {}", p.display()),
                Ok(None) => "prev: none".into(),
                Err(e) => format!("prev error: {e}"),
            },
            "pause" | "toggle-pause" => {
                self.ctx.toggle_pause()?;
                format!("paused: {}", self.ctx.state.paused)
            }
            "status" => format!(
                "paused={} history={} queue={}",
                self.ctx.state.paused,
                self.ctx.state.history.len(),
                self.ctx.state.cache_queue.len()
            ),
            "quit" | "q" => return Ok(None),
            "" => "(empty command)".into(),
            other => format!("unknown command: {other} (try :next :prev :pause :status :quit)"),
        };
        Ok(Some(msg))
    }

    pub fn footer_help(&self) -> String {
        let keys = match self.input_mode {
            InputMode::Command => format!(":{}_ | Enter run Esc cancel", self.cmd_line),
            InputMode::SearchInput => {
                "Search: type query | Enter search Esc cancel | i".to_string()
            }
            InputMode::Normal => match self.tab {
                Tab::Search => {
                    "5 Search | i edit query Enter search | j/k | Enter apply | : cmd".into()
                }
                _ => "1-5 tabs | n/p next/prev | f favorite d trash | space pause | : cmd".into(),
            },
        };
        format!("{keys} | q quit | {}", self.message)
    }
}

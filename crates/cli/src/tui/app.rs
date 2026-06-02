use std::path::PathBuf;

use walls_core::apply::ApplyTrigger;
use walls_core::WallsCtx;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Status,
    Now,
    History,
    Browse,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Status => 0,
            Tab::Now => 1,
            Tab::History => 2,
            Tab::Browse => 3,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Status => "Status",
            Tab::Now => "Now",
            Tab::History => "History",
            Tab::Browse => "Browse",
        }
    }
}

pub struct App {
    pub ctx: WallsCtx,
    pub tab: Tab,
    pub cursor: usize,
    pub message: String,
}

impl App {
    pub fn new(ctx: WallsCtx) -> anyhow::Result<Self> {
        Ok(Self {
            ctx,
            tab: Tab::Status,
            cursor: 0,
            message: String::new(),
        })
    }

    pub fn reload_ctx(&mut self) -> anyhow::Result<()> {
        let paths = self.ctx.paths.clone();
        self.ctx = WallsCtx::load_with_paths(paths)?;
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

    fn list_len(&self) -> usize {
        match self.tab {
            Tab::History => self.ctx.state.history.len(),
            Tab::Browse => self.browse_items().len(),
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

    fn browse_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        items.push("-- cache queue --".into());
        for id in &self.ctx.state.cache_queue {
            items.push(format!("queue: {id}"));
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
            self.ctx.state.cache_queue.retain(|q| q != id);
            self.ctx.state.cache_queue.insert(0, id.to_string());
            self.ctx.save_state()?;
            if let Some(p) = self.ctx.advance_next().await? {
                return Ok(Some(format!("applied queue head: {}", p.display())));
            }
            return Ok(Some("queue item not applicable".into()));
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

    pub fn footer_help(&self) -> String {
        format!(
            "1-4 tabs | n next p prev | space pause | j/k move | enter apply | q quit | {}",
            self.message
        )
    }
}
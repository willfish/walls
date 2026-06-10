use std::path::PathBuf;

use super::{wallhaven_edit, App, EditTarget, Tab};
use crate::tui::open_target::{self, OpenTarget};

impl App {
    pub(crate) fn selected_open_target(&self) -> Option<OpenTarget> {
        match self.tab {
            Tab::Config => self.selected_config_open_target(),
            Tab::Now => self
                .ctx
                .current_path()
                .map(|path| OpenTarget::Path(path.to_path_buf())),
            Tab::History => self.selected_history_open_target(),
            Tab::Browse => self.selected_browse_open_target(),
            Tab::Search => self.selected_search_open_target(),
            Tab::Logs => None,
        }
    }

    fn selected_config_open_target(&self) -> Option<OpenTarget> {
        if !self.is_sources_list_block(self.config_cursor) {
            return None;
        }

        let target = if self.config_in_subnav {
            self.selected_sources_subnav_edit_target()
        } else {
            self.default_sources_edit_target()
        }?;

        self.open_target_for_edit_target(&target)
    }

    fn open_target_for_edit_target(&self, target: &EditTarget) -> Option<OpenTarget> {
        match target {
            EditTarget::Source(index) => self
                .ctx
                .config
                .sources
                .get(*index)
                .and_then(|source| open_target::source(&self.ctx, source)),
            EditTarget::Wallhaven => Some(open_target::wallhaven_search(
                &wallhaven_edit::first_search(&self.ctx.config),
            )),
            _ => None,
        }
    }

    fn selected_history_open_target(&self) -> Option<OpenTarget> {
        self.ctx
            .state
            .history
            .get(self.cursor)
            .map(PathBuf::from)
            .map(OpenTarget::Path)
    }

    fn selected_browse_open_target(&self) -> Option<OpenTarget> {
        let line = self.browse_items().get(self.cursor)?.clone();
        if let Some(path) = line
            .strip_prefix("local: ")
            .or_else(|| line.strip_prefix("history: "))
        {
            return Some(OpenTarget::Path(PathBuf::from(path)));
        }

        let id = line.strip_prefix("queue: ")?;
        self.open_target_for_cache_queue_id(id)
    }

    fn selected_search_open_target(&self) -> Option<OpenTarget> {
        let hit = self.search_results.get(self.cursor)?;
        Some(open_target::wallhaven_wallpaper(&hit.id))
    }

    fn open_target_for_cache_queue_id(&self, id: &str) -> Option<OpenTarget> {
        Some(open_target::cache_queue_id(&self.ctx.paths.cache_dir, id))
    }
}

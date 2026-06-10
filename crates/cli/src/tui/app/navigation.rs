use super::{App, Tab, CONFIG_BLOCK_SOURCES};

impl App {
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
        let len = crate::tui::log_len();
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
            Tab::Logs => crate::tui::log_len(),
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

    pub fn is_sources_list_block(&self, block: usize) -> bool {
        block == CONFIG_BLOCK_SOURCES
    }
}

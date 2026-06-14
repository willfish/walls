use super::{App, EditFieldKind, InputMode, Tab};

impl App {
    pub fn footer_keys(&self) -> String {
        if self.show_key_help {
            return "Key help | Esc/q close".into();
        }
        if self.is_editing() {
            let choice_hint = if self.current_edit_field_locked() {
                if self
                    .editing
                    .as_ref()
                    .and_then(|s| s.draft_source.as_ref())
                    .is_some_and(|src| src.source_type == "reddit")
                {
                    "top/controversial only"
                } else {
                    "locked"
                }
            } else {
                match self.current_edit_field_kind() {
                    EditFieldKind::Text => "type/Backspace",
                    EditFieldKind::TagList if self.tag_input_active() => {
                        "type tag | Backspace | Enter commit | Esc cancel"
                    }
                    EditFieldKind::TagList if self.tag_editor_active() => {
                        "←/→ tags | x delete | a add | e edit | Esc fields"
                    }
                    EditFieldKind::TagList => "Enter tags",
                    EditFieldKind::Bool => "Space toggle",
                    EditFieldKind::Choice(_) => "Space/←/→ cycle",
                }
            };
            return format!("edit: ↑/↓ fields | {choice_hint} | Enter save | Esc cancel | q");
        }
        if self.pending_trash_confirm {
            return "d confirm trash current wallpaper | Esc cancel".into();
        }
        if self.pending_nuke_confirm {
            return "Shift+X confirm provider reset | Esc cancel".into();
        }
        let keys = match self.input_mode {
            InputMode::Command => {
                format!(
                    ":{}_ | Ctrl+n/p complete | Enter run Esc cancel",
                    self.cmd_line
                )
            }
            InputMode::SearchInput => "Search: type query | Enter search Esc cancel".to_string(),
            InputMode::Normal => match self.tab {
                Tab::Search => {
                    let enter_hint = if self.search_results.is_empty() {
                        "Enter search"
                    } else {
                        "Enter apply"
                    };
                    format!(
                        "{} | / or i query | e filters | o open | {enter_hint} | j/k Pg Home/End | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
                Tab::Config => {
                    if self.config_in_subnav && self.is_sources_list_block(self.config_cursor) {
                        let remove_hint = if self.can_remove_selected_source() {
                            " | x remove"
                        } else {
                            ""
                        };
                        format!(
                            "{} | Esc back | j/k Pg Home/End pick source | a add{} | o open | e edit | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT,
                            remove_hint
                        )
                    } else if self.is_sources_list_block(self.config_cursor) {
                        format!(
                            "{} | j/k Pg Home/End | a add | o open | e first active | Enter pick | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT
                        )
                    } else {
                        format!(
                            "{} | j/k Pg Home/End | e edit | t toggle | n/p | space pause | : cmd | ? help",
                            Self::NORMAL_TAB_NAV_HINT
                        )
                    }
                }
                Tab::Logs => {
                    format!(
                        "{} | newest first | j older k newer | Home newest End oldest | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
                Tab::Now => {
                    let create_hint = if self.current_has_wallhaven_id() {
                        " | c create source"
                    } else {
                        ""
                    };
                    format!(
                        "{} | o open | f favorite | d trash{} | ? help",
                        Self::NORMAL_TAB_NAV_HINT,
                        create_hint
                    )
                }
                _ => {
                    format!(
                        "{} | j/k Pg Home/End | o open | n/p next/prev | f favorite d request trash | Shift+X reset | space pause | : cmd | ? help",
                        Self::NORMAL_TAB_NAV_HINT
                    )
                }
            },
        };
        format!("{keys} | q quit")
    }

    pub fn compact_footer_keys(&self) -> String {
        if self.show_key_help {
            return "help | Esc/q close".into();
        }
        if self.is_editing() {
            return "edit | ↑/↓ fields | Space/←/→ | Enter | Esc | q".into();
        }
        if self.pending_trash_confirm {
            return "d confirm trash | Esc cancel".into();
        }
        if self.pending_nuke_confirm {
            return "Shift+X confirm reset | Esc cancel".into();
        }
        match self.input_mode {
            InputMode::Command => format!(":{}_ | Enter | Esc | q", self.cmd_line),
            InputMode::SearchInput => "type | Enter search | Esc | q".into(),
            InputMode::Normal => {
                let nav = Self::NORMAL_TAB_NAV_HINT;
                match self.tab {
                    Tab::Search => {
                        let enter_hint = if self.search_results.is_empty() {
                            "Enter search"
                        } else {
                            "Enter apply"
                        };
                        format!("{nav} /i e o {enter_hint} j/k :?q")
                    }
                    Tab::Config
                        if self.config_in_subnav
                            && self.is_sources_list_block(self.config_cursor) =>
                    {
                        format!("{nav} Esc j/k Pg o e t n/p sp :?q")
                    }
                    Tab::Config => {
                        format!("{nav} j/k Pg Enter o e t n/p sp :?q")
                    }
                    Tab::Logs => format!("{nav} newest j older k newer :?q"),
                    Tab::Now => {
                        let create_hint = if self.current_has_wallhaven_id() {
                            " c"
                        } else {
                            ""
                        };
                        format!("{nav} o f d{create_hint} ?q")
                    }
                    _ => format!("{nav} j/k Pg o n/p f/d? Shift+X sp :?q"),
                }
            }
        }
    }

    fn current_has_wallhaven_id(&self) -> bool {
        self.ctx
            .state
            .current
            .as_ref()
            .and_then(|current| current.wallhaven_id.as_deref())
            .is_some_and(|id| !id.trim().is_empty())
    }
}

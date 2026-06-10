use walls_core::config::{
    default_wallhaven_source, normalize_source_entry, persist_config, Config, SourceEntry,
};
use walls_core::validate::{
    validate_config_diagnostics, validate_source_edit, validate_wallhaven_edit,
};

use super::edit_fields::{
    block_field_kind, block_field_value_at, choice_display_value, commit_block_field_buffer,
    cycle_choice_value, default_wallhaven_source_entry, reddit_time_field_locked,
    search_filter_field_value_at, source_entry_display_name, source_field_kind_for,
    source_removal_protected, toggle_bool_value, APPLY_DISPLAY_BLOCK_FIELDS,
    CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY, CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_SOURCES,
    CONFIG_BLOCK_TUI, LIBRARY_BLOCK_FIELDS, ROTATION_BLOCK_FIELDS, SEARCH_FILTER_FIELDS,
    TUI_BLOCK_FIELDS, WALLHAVEN_BLOCK_FIELDS, WALLHAVEN_FIELDS_BLOCK,
};
use super::{
    config_block_edit, source_edit, wallhaven_edit, App, EditFieldKind, EditSession, EditTarget,
    InputMode, Tab,
};
use crate::tui::style::StatusKind;

impl App {
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

    pub(super) fn selected_sources_subnav_edit_target(&self) -> Option<EditTarget> {
        let idx = self.config_sub_cursor;
        if idx < self.ctx.config.sources.len() {
            Some(EditTarget::Source(idx))
        } else {
            None
        }
    }

    pub(super) fn default_sources_edit_target(&self) -> Option<EditTarget> {
        self.ctx
            .config
            .sources
            .iter()
            .position(|source| source.enabled)
            .map(EditTarget::Source)
    }

    pub(super) fn edit_session_for_target(&self, target: EditTarget) -> Option<EditSession> {
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

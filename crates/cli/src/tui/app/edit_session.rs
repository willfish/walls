use walls_core::config::{
    default_wallhaven_source, normalize_source_entry, persist_config, Config, SourceEntry,
    WallhavenPrefer,
};
use walls_core::validate::{
    validate_config_diagnostics, validate_source_edit, validate_wallhaven_edit,
};

use super::edit_fields::{
    block_field_kind, block_field_value_at, choice_display_value, commit_block_field_buffer,
    cycle_choice_value, default_wallhaven_source_entry, reddit_time_field_locked,
    search_filter_field_value_at, source_entry_display_name, source_removal_protected,
    toggle_bool_value, APPLY_DISPLAY_BLOCK_FIELDS, CONFIG_BLOCK_APPLY_DISPLAY,
    CONFIG_BLOCK_LIBRARY, CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_SOURCES, CONFIG_BLOCK_TUI,
    LIBRARY_BLOCK_FIELDS, ROTATION_BLOCK_FIELDS, SEARCH_FILTER_FIELDS, TUI_BLOCK_FIELDS,
    WALLHAVEN_BLOCK_FIELDS, WALLHAVEN_FIELDS_BLOCK,
};
use super::{
    config_block_edit, source_edit, source_field_schema, wallhaven_edit, App, EditFieldKind,
    EditSession, EditTarget, InputMode, Tab, TagEditor, TagEditorMode,
};
use crate::tui::style::StatusKind;

impl App {
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
        self.sync_edit_field_buffer();
        self.clear_message();
    }

    pub fn start_search_filter_edit(&mut self) {
        self.tab = Tab::Search;
        self.input_mode = InputMode::Normal;
        self.config_in_subnav = false;
        self.editing = self.edit_session_for_target(EditTarget::SearchFilters);
        self.sync_edit_field_buffer();
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
                Some(EditSession::new(
                    target.clone(),
                    Some(draft),
                    std::collections::HashMap::new(),
                ))
            }
            EditTarget::Block(CONFIG_BLOCK_ROTATION) => Some(EditSession::new(
                target.clone(),
                None,
                config_block_edit::rotation_draft(&self.ctx.config),
            )),
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => Some(EditSession::new(
                target.clone(),
                None,
                config_block_edit::library_draft(&self.ctx.config),
            )),
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => Some(EditSession::new(
                target.clone(),
                None,
                config_block_edit::display_draft(&self.ctx.config),
            )),
            EditTarget::Block(CONFIG_BLOCK_TUI) => Some(EditSession::new(
                target.clone(),
                None,
                config_block_edit::tui_draft(&self.ctx.config),
            )),
            EditTarget::Wallhaven => Some(EditSession::new(
                target.clone(),
                None,
                wallhaven_edit::block_draft(
                    &self.ctx.config,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                ),
            )),
            EditTarget::SearchFilters => Some(EditSession::new(
                target.clone(),
                None,
                wallhaven_edit::search_draft(
                    &self.search_filters,
                    wallhaven_edit::api_key_present(&self.ctx.secrets),
                ),
            )),
            _ => None,
        }
    }

    fn sync_edit_field_buffer(&mut self) {
        let new_buf = self.current_edit_field_value();
        if let Some(session) = &mut self.editing {
            session.field_buffer = new_buf;
            session.tag_editor = None;
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.set_message(StatusKind::Neutral, "edit cancelled");
    }

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
                source_field_schema::source_field_specs(src).len()
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
                let specs = source_field_schema::source_field_specs(src);
                specs
                    .get(sess.field_cursor)
                    .map_or(EditFieldKind::Text, |spec| spec.kind)
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
                let specs = source_field_schema::source_field_specs(draft);
                if let Some(spec) = specs.get(sess.field_cursor) {
                    let name = spec.key.as_str();
                    if draft.source_type == "wallhaven" && name == "purity_nsfw" {
                        return self.wallhaven_block_field_locked(name);
                    }
                    return name == "time" && reddit_time_field_locked(draft);
                }
            }
        }
        false
    }

    pub(crate) fn tag_editor_active(&self) -> bool {
        self.editing
            .as_ref()
            .is_some_and(|session| session.tag_editor.is_some())
    }

    pub(crate) fn tag_input_active(&self) -> bool {
        self.editing.as_ref().is_some_and(|session| {
            session.tag_editor.as_ref().is_some_and(|editor| {
                matches!(editor.mode, TagEditorMode::Add | TagEditorMode::Edit)
            })
        })
    }

    pub(crate) fn enter_tag_editor(&mut self) {
        if self.current_edit_field_kind() != EditFieldKind::TagList {
            return;
        }
        let max = self.current_tag_values().len();
        if let Some(session) = &mut self.editing {
            let mut editor = TagEditor::browse();
            if max > 0 {
                editor.tag_cursor = editor.tag_cursor.min(max - 1);
            }
            session.tag_editor = Some(editor);
        }
    }

    pub(crate) fn exit_tag_editor(&mut self) {
        if let Some(session) = &mut self.editing {
            session.tag_editor = None;
        }
    }

    pub(crate) fn move_tag_cursor(&mut self, forward: bool) {
        let max = self.current_tag_values().len();
        if max == 0 {
            return;
        }
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.tag_cursor = if forward {
                (editor.tag_cursor + 1).min(max - 1)
            } else {
                editor.tag_cursor.saturating_sub(1)
            };
        }
    }

    pub(crate) fn delete_current_tag(&mut self) {
        let Some(cursor) = self.current_tag_cursor() else {
            return;
        };
        let mut tags = self.current_tag_values();
        if cursor >= tags.len() {
            return;
        }
        tags.remove(cursor);
        self.set_current_tag_values(tags);
        let new_len = self.current_tag_values().len();
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.tag_cursor = editor.tag_cursor.min(new_len.saturating_sub(1));
            editor.mode = TagEditorMode::Browse;
            editor.input.clear();
        }
        let _ = self.save_edit_item(false);
    }

    pub(crate) fn begin_add_tag(&mut self) {
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.mode = TagEditorMode::Add;
            editor.input.clear();
        }
    }

    pub(crate) fn begin_edit_tag(&mut self) {
        let Some(cursor) = self.current_tag_cursor() else {
            return;
        };
        let tags = self.current_tag_values();
        let Some(tag) = tags.get(cursor).cloned() else {
            return;
        };
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.mode = TagEditorMode::Edit;
            editor.input = tag;
        }
    }

    pub(crate) fn tag_input_char(&mut self, c: char) {
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.input.push(c);
        }
    }

    pub(crate) fn tag_input_backspace(&mut self) {
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.input.pop();
        }
    }

    pub(crate) fn cancel_tag_input(&mut self) {
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.mode = TagEditorMode::Browse;
            editor.input.clear();
        }
    }

    pub(crate) fn commit_tag_input(&mut self) {
        let Some((mode, cursor, input)) = self
            .editing
            .as_ref()
            .and_then(|session| session.tag_editor.as_ref())
            .map(|editor| (editor.mode.clone(), editor.tag_cursor, editor.input.clone()))
        else {
            return;
        };
        let tag = input.trim();
        if tag.is_empty() {
            self.cancel_tag_input();
            return;
        }
        let mut tags = self.current_tag_values();
        match mode {
            TagEditorMode::Add => {
                tags.push(tag.to_string());
            }
            TagEditorMode::Edit if cursor < tags.len() => {
                tags[cursor] = tag.to_string();
            }
            TagEditorMode::Browse | TagEditorMode::Edit => return,
        }
        tags = walls_core::config::normalize_wallhaven_tags(&tags);
        let selected = tags
            .iter()
            .position(|candidate| candidate == tag)
            .unwrap_or_else(|| cursor.min(tags.len().saturating_sub(1)));
        self.set_current_tag_values(tags);
        if let Some(editor) = self
            .editing
            .as_mut()
            .and_then(|session| session.tag_editor.as_mut())
        {
            editor.tag_cursor = selected;
            editor.mode = TagEditorMode::Browse;
            editor.input.clear();
        }
        let _ = self.save_edit_item(false);
    }

    pub(crate) fn tag_editor_display_value(&self) -> Option<String> {
        let session = self.editing.as_ref()?;
        let editor = session.tag_editor.as_ref()?;
        let tags = self.current_tag_values();
        let rendered = if tags.is_empty() {
            "(none)".to_string()
        } else {
            tags.iter()
                .enumerate()
                .map(|(index, tag)| {
                    if index == editor.tag_cursor && editor.mode == TagEditorMode::Browse {
                        format!("[{tag}]")
                    } else {
                        tag.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        Some(match editor.mode {
            TagEditorMode::Browse => format!("{rendered}  (←/→ select, x delete, a add, e edit)"),
            TagEditorMode::Add => format!("{rendered}  +{}|", editor.input),
            TagEditorMode::Edit => format!("{rendered}  edit: {}|", editor.input),
        })
    }

    fn current_tag_cursor(&self) -> Option<usize> {
        self.editing
            .as_ref()
            .and_then(|session| session.tag_editor.as_ref())
            .map(|editor| editor.tag_cursor)
    }

    fn current_source_field_name(&self) -> Option<String> {
        let session = self.editing.as_ref()?;
        let draft = session.draft_source.as_ref()?;
        let specs = source_field_schema::source_field_specs(draft);
        specs.get(session.field_cursor).map(|spec| spec.key.clone())
    }

    fn current_tag_values(&self) -> Vec<String> {
        let Some(name) = self.current_source_field_name() else {
            return Vec::new();
        };
        let Some(draft) = self
            .editing
            .as_ref()
            .and_then(|session| session.draft_source.as_ref())
        else {
            return Vec::new();
        };
        match name.as_str() {
            "required_tags" => draft.required_tags.clone(),
            "excluded_tags" => draft.excluded_tags.clone(),
            _ => Vec::new(),
        }
    }

    fn set_current_tag_values(&mut self, tags: Vec<String>) {
        let Some(name) = self.current_source_field_name() else {
            return;
        };
        let normalized = walls_core::config::normalize_wallhaven_tags(&tags);
        let joined = normalized.join(", ");
        if let Some(draft) = self
            .editing
            .as_mut()
            .and_then(|session| session.draft_source.as_mut())
        {
            match name.as_str() {
                "required_tags" => draft.required_tags = normalized,
                "excluded_tags" => draft.excluded_tags = normalized,
                _ => return,
            }
        }
        if let Some(session) = &mut self.editing {
            session.field_buffer = joined;
        }
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
            EditFieldKind::Text | EditFieldKind::TagList => return,
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

    #[cfg(test)]
    pub fn source_editable_fields(src: &walls_core::config::SourceEntry) -> Vec<String> {
        source_field_schema::source_field_specs(src)
            .into_iter()
            .map(|spec| spec.key)
            .collect()
    }

    pub(super) fn parse_bool_like(s: &str) -> Option<bool> {
        source_edit::parse_bool_like(s)
    }

    /// Pure value lookup for a field at a given cursor idx for a target (no reliance on live editing sess cursor).
    /// Used by up/down handlers to precompute the *new* position's buffer value without borrow conflicts.
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
                let specs = source_field_schema::source_field_specs(src);
                specs.get(idx).map_or_else(String::new, |spec| {
                    source_field_schema::source_field_value(src, &spec.key)
                })
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
                    let specs = source_field_schema::source_field_specs(src);
                    specs.get(idx).map_or_else(String::new, |spec| {
                        source_field_schema::source_field_value(src, &spec.key)
                    })
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
        self.sync_edit_field_buffer();
        self.set_message(StatusKind::Success, "source added: Wallhaven query");
        Ok(())
    }

    pub fn add_wallhaven_source_from_current(
        &mut self,
        rt: &tokio::runtime::Handle,
    ) -> anyhow::Result<(String, StatusKind)> {
        let Some(wallhaven_id) = self
            .ctx
            .state
            .current
            .as_ref()
            .and_then(|current| current.wallhaven_id.as_deref())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
        else {
            return Ok((
                "source from-current: current wallpaper has no Wallhaven id".into(),
                StatusKind::Warning,
            ));
        };

        let client = walls_core::wallhaven::WallhavenClient::new(
            walls_core::wallhaven::api_base(),
            &self.ctx.secrets.wallhaven_api_key,
        )?;
        let wallpaper =
            tokio::task::block_in_place(|| rt.block_on(client.fetch_wallpaper(&wallhaven_id)))?;
        let tags = walls_core::wallhaven::tag_names_from_wallpaper(&wallpaper);
        if tags.is_empty() {
            return Ok((
                format!("source from-current: Wallhaven {wallhaven_id} has no usable tags"),
                StatusKind::Warning,
            ));
        }
        let query = tags
            .iter()
            .filter_map(|tag| walls_core::config::wallhaven_required_tag_query_part(tag))
            .collect::<Vec<_>>()
            .join(" ");

        let mut source = default_wallhaven_source_entry();
        source.query = Some(String::new());
        source.required_tags = tags;
        source.atleast = Some(String::new());
        source.ratios = Some(String::new());
        source.prefer = Some(WallhavenPrefer::SearchOnly);
        normalize_source_entry(&mut source);

        let mut config = self.ctx.config.clone();
        config.sources.push(source);
        persist_config(&self.ctx.paths.config_file, &config)?;
        self.reload_ctx()?;
        Ok((
            format!("source added: Wallhaven tags {query}"),
            StatusKind::Success,
        ))
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

    pub fn commit_edit_field_buffer(&mut self) {
        if let Some(sess) = &mut self.editing {
            let buf = std::mem::take(&mut sess.field_buffer);
            let field_idx = sess.field_cursor;
            match &mut sess.target {
                EditTarget::Source(i) if *i < self.ctx.config.sources.len() => {
                    if let Some(draft) = &mut sess.draft_source {
                        let specs = source_field_schema::source_field_specs(draft);
                        if let Some(spec) = specs.get(field_idx) {
                            let name = &spec.key;
                            let prev_type = draft.source_type.clone();
                            source_field_schema::set_source_field_value(draft, name, &buf);
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

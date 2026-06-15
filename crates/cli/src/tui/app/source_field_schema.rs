use std::collections::HashMap;

use walls_core::config::Config;
use walls_core::config::SourceEntry;
use walls_core::validate::validate_config_diagnostics;

use super::edit_fields::{
    block_field_kind, block_field_label, block_field_value_at, reddit_time_field_locked,
    source_field_kind_for, source_field_label, EditFieldKind, APPLY_DISPLAY_BLOCK_FIELDS,
    CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY, CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_TUI,
    LIBRARY_BLOCK_FIELDS, ROTATION_BLOCK_FIELDS, TUI_BLOCK_FIELDS,
};
use super::{config_block_edit, source_edit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditFieldSpec {
    pub key: String,
    pub label: String,
    pub kind: EditFieldKind,
    pub locked: bool,
    pub persistence_target: EditPersistenceTarget,
    pub validation_scope: EditValidationScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditPersistenceTarget {
    Source,
    Rotation,
    Library,
    ApplyDisplay,
    Tui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditValidationScope {
    Source,
    None,
    Library,
    ApplyDisplay,
}

impl EditFieldSpec {
    pub(crate) fn value(&self, config: &Config, draft: &HashMap<String, String>) -> String {
        let Some(block) = self.persistence_target.block_index() else {
            return String::new();
        };
        let idx = block_field_keys(block)
            .iter()
            .position(|key| *key == self.key)
            .unwrap_or(usize::MAX);
        block_field_value_at(config, block, draft, idx)
    }

    pub(crate) fn commit(&self, value: &str, draft: &mut HashMap<String, String>) {
        if self.persistence_target == EditPersistenceTarget::Source {
            return;
        }
        draft.insert(self.key.clone(), value.trim().to_string());
    }
}

impl EditPersistenceTarget {
    pub(crate) fn block_index(self) -> Option<usize> {
        match self {
            Self::Source => None,
            Self::Rotation => Some(CONFIG_BLOCK_ROTATION),
            Self::Library => Some(CONFIG_BLOCK_LIBRARY),
            Self::ApplyDisplay => Some(CONFIG_BLOCK_APPLY_DISPLAY),
            Self::Tui => Some(CONFIG_BLOCK_TUI),
        }
    }
}

pub(crate) fn source_field_specs(source: &SourceEntry) -> Vec<EditFieldSpec> {
    source_edit::source_editable_fields(source)
        .into_iter()
        .map(|key| EditFieldSpec {
            label: source_field_label(source, &key),
            kind: source_field_kind_for(source, &key),
            locked: key == "time" && reddit_time_field_locked(source),
            persistence_target: EditPersistenceTarget::Source,
            validation_scope: EditValidationScope::Source,
            key,
        })
        .collect()
}

pub(crate) fn block_field_specs(block: usize) -> Vec<EditFieldSpec> {
    block_field_keys(block)
        .iter()
        .map(|key| EditFieldSpec {
            key: (*key).to_string(),
            label: block_field_label(block, key),
            kind: block_field_kind(block, key),
            locked: block_field_locked(block, key),
            persistence_target: block_persistence_target(block),
            validation_scope: block_validation_scope(block),
        })
        .collect()
}

pub(crate) fn block_field_keys(block: usize) -> &'static [&'static str] {
    match block {
        CONFIG_BLOCK_ROTATION => ROTATION_BLOCK_FIELDS,
        CONFIG_BLOCK_LIBRARY => LIBRARY_BLOCK_FIELDS,
        CONFIG_BLOCK_APPLY_DISPLAY => APPLY_DISPLAY_BLOCK_FIELDS,
        CONFIG_BLOCK_TUI => TUI_BLOCK_FIELDS,
        _ => &[],
    }
}

pub(crate) fn block_field_spec_at(block: usize, index: usize) -> Option<EditFieldSpec> {
    block_field_specs(block).into_iter().nth(index)
}

pub(crate) fn apply_block_draft(
    target: EditPersistenceTarget,
    config: &mut Config,
    draft: &HashMap<String, String>,
) -> Option<&'static str> {
    match target {
        EditPersistenceTarget::Rotation => {
            config_block_edit::apply_rotation_draft(config, draft);
            Some("config saved: rotation")
        }
        EditPersistenceTarget::Library => {
            config_block_edit::apply_library_draft(config, draft);
            Some("config saved: library")
        }
        EditPersistenceTarget::ApplyDisplay => {
            config_block_edit::apply_display_draft(config, draft);
            Some("config saved: display")
        }
        EditPersistenceTarget::Tui => {
            config_block_edit::apply_tui_draft(config, draft);
            Some("config saved: tui preferences")
        }
        EditPersistenceTarget::Source => None,
    }
}

pub(crate) fn validation_issues(
    scope: EditValidationScope,
    config: &Config,
    secrets: &walls_core::config::Secrets,
    paths: &walls_core::paths::WallsPaths,
) -> Vec<String> {
    match scope {
        EditValidationScope::Library => validate_config_diagnostics(config, secrets, paths)
            .into_iter()
            .filter(|diagnostic| diagnostic.path.starts_with("quota."))
            .map(|diagnostic| diagnostic.to_string())
            .collect(),
        EditValidationScope::ApplyDisplay => {
            let mut issues: Vec<String> = validate_config_diagnostics(config, secrets, paths)
                .into_iter()
                .filter(|diagnostic| diagnostic.path.starts_with("apply."))
                .map(|diagnostic| diagnostic.to_string())
                .collect();
            if config.display.target_width.is_some() ^ config.display.target_height.is_some() {
                issues.push(
                    "display target: set both target_width and target_height, or clear both".into(),
                );
            }
            issues
        }
        EditValidationScope::Source | EditValidationScope::None => Vec::new(),
    }
}

pub(crate) fn source_field_value(source: &SourceEntry, key: &str) -> String {
    source_edit::get_source_field(source, key)
}

pub(crate) fn set_source_field_value(source: &mut SourceEntry, key: &str, value: &str) {
    source_edit::set_source_field(source, key, value);
}

fn block_persistence_target(block: usize) -> EditPersistenceTarget {
    match block {
        CONFIG_BLOCK_ROTATION => EditPersistenceTarget::Rotation,
        CONFIG_BLOCK_LIBRARY => EditPersistenceTarget::Library,
        CONFIG_BLOCK_APPLY_DISPLAY => EditPersistenceTarget::ApplyDisplay,
        CONFIG_BLOCK_TUI => EditPersistenceTarget::Tui,
        _ => EditPersistenceTarget::Source,
    }
}

fn block_validation_scope(block: usize) -> EditValidationScope {
    match block {
        CONFIG_BLOCK_LIBRARY => EditValidationScope::Library,
        CONFIG_BLOCK_APPLY_DISPLAY => EditValidationScope::ApplyDisplay,
        CONFIG_BLOCK_ROTATION | CONFIG_BLOCK_TUI => EditValidationScope::None,
        _ => EditValidationScope::Source,
    }
}

fn block_field_locked(block: usize, key: &str) -> bool {
    block == CONFIG_BLOCK_ROTATION
        && key == "tray_autostart"
        && !walls_core::autostart::tray_autostart_available(
            walls_core::autostart::current_autostart_desktop(),
        )
}

#[cfg(test)]
mod tests {
    use walls_core::config::SourceEntry;

    use super::{EditFieldKind, CONFIG_BLOCK_APPLY_DISPLAY};
    use crate::tui::app::APPLY_BACKEND_CHOICES;

    #[test]
    fn source_field_specs_include_ordered_metadata_and_value_access() {
        let mut source = SourceEntry {
            enabled: true,
            source_type: "reddit".into(),
            query: Some("unixporn".into()),
            sort: Some("top".into()),
            time: Some("month".into()),
            ..SourceEntry::default()
        };

        let specs = super::source_field_specs(&source);

        assert_eq!(
            specs
                .iter()
                .map(|spec| spec.key.as_str())
                .collect::<Vec<_>>(),
            walls_core::config::source_editable_fields(&source)
        );
        assert_eq!(specs[1].label, "Subreddit");
        assert_eq!(
            specs[2].kind,
            EditFieldKind::Choice(walls_core::config::REDDIT_SORT_CHOICES)
        );
        assert_eq!(super::source_field_value(&source, "query"), "unixporn");

        super::set_source_field_value(&mut source, "query", "earthporn");

        assert_eq!(source.query.as_deref(), Some("earthporn"));
    }

    #[test]
    fn block_field_specs_include_ordered_metadata() {
        let specs = super::block_field_specs(CONFIG_BLOCK_APPLY_DISPLAY);

        assert_eq!(specs[0].key, "apply_backend");
        assert_eq!(specs[0].label, "Apply backend");
        assert_eq!(specs[0].kind, EditFieldKind::Choice(APPLY_BACKEND_CHOICES));
        assert_eq!(specs[1].key, "custom_script");
        assert_eq!(specs[1].label, "Custom script");
        assert_eq!(specs[1].kind, EditFieldKind::Text);
    }

    #[test]
    fn source_field_specs_include_lock_and_save_metadata() {
        let source = SourceEntry {
            enabled: true,
            source_type: "reddit".into(),
            query: Some("unixporn".into()),
            sort: Some("hot".into()),
            time: Some("month".into()),
            ..SourceEntry::default()
        };

        let specs = super::source_field_specs(&source);
        let time = specs
            .iter()
            .find(|spec| spec.key == "time")
            .expect("time spec");

        assert!(time.locked);
        assert_eq!(
            time.persistence_target,
            super::EditPersistenceTarget::Source
        );
        assert_eq!(time.validation_scope, super::EditValidationScope::Source);
    }

    #[test]
    fn block_field_specs_include_value_commit_lock_and_save_metadata() {
        let mut config = walls_core::config::default_config().expect("default config");
        config.display.auto_rotate = true;
        let mut draft = std::collections::HashMap::new();
        let specs = super::block_field_specs(CONFIG_BLOCK_APPLY_DISPLAY);
        let auto_rotate = specs
            .iter()
            .find(|spec| spec.key == "auto_rotate")
            .expect("auto rotate spec");

        assert_eq!(auto_rotate.value(&config, &draft), "true");
        assert!(!auto_rotate.locked);
        assert_eq!(
            auto_rotate.persistence_target,
            super::EditPersistenceTarget::ApplyDisplay
        );
        assert_eq!(
            auto_rotate.validation_scope,
            super::EditValidationScope::ApplyDisplay
        );

        auto_rotate.commit("false", &mut draft);

        assert_eq!(draft.get("auto_rotate").map(String::as_str), Some("false"));
    }
}

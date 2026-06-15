use walls_core::config::SourceEntry;

use super::edit_fields::{
    block_field_kind, block_field_label, source_field_kind_for, source_field_label, EditFieldKind,
    APPLY_DISPLAY_BLOCK_FIELDS, CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY,
    CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_TUI, LIBRARY_BLOCK_FIELDS, ROTATION_BLOCK_FIELDS,
    TUI_BLOCK_FIELDS,
};
use super::source_edit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditFieldSpec {
    pub key: String,
    pub label: String,
    pub kind: EditFieldKind,
}

pub(crate) fn source_field_specs(source: &SourceEntry) -> Vec<EditFieldSpec> {
    source_edit::source_editable_fields(source)
        .into_iter()
        .map(|key| EditFieldSpec {
            label: source_field_label(source, &key),
            kind: source_field_kind_for(source, &key),
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

pub(crate) fn source_field_value(source: &SourceEntry, key: &str) -> String {
    source_edit::get_source_field(source, key)
}

pub(crate) fn set_source_field_value(source: &mut SourceEntry, key: &str, value: &str) {
    source_edit::set_source_field(source, key, value);
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
}

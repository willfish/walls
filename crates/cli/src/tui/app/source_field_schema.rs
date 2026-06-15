use walls_core::config::SourceEntry;

use super::edit_fields::{source_field_kind_for, source_field_label, EditFieldKind};
use super::source_edit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFieldSpec {
    pub key: String,
    pub label: String,
    pub kind: EditFieldKind,
}

pub(crate) fn source_field_specs(source: &SourceEntry) -> Vec<SourceFieldSpec> {
    source_edit::source_editable_fields(source)
        .into_iter()
        .map(|key| SourceFieldSpec {
            label: source_field_label(source, &key),
            kind: source_field_kind_for(source, &key),
            key,
        })
        .collect()
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

    use super::EditFieldKind;

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
}

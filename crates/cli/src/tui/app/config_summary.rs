use walls_core::config::SourceEntry;
use walls_core::expand_home;
use walls_core::sources::list_images_with_paths;
use walls_core::validate::validate_config_diagnostics;
use walls_core::WallsCtx;

pub struct LocalSourceSummary {
    pub enabled: bool,
    pub source_type: String,
    pub label: String,
    pub path: String,
    pub status: String,
    pub candidates: usize,
}

pub(super) fn is_local_source(source: &SourceEntry) -> bool {
    matches!(
        source.source_type.as_str(),
        "folder" | "image" | "favorites" | "fetched"
    )
}

pub(super) fn summarize_local_source(ctx: &WallsCtx, source: &SourceEntry) -> LocalSourceSummary {
    let label = source
        .label
        .clone()
        .unwrap_or_else(|| source.source_type.clone());
    let path = match source.source_type.as_str() {
        "favorites" => Some(ctx.paths.favorites_dir.clone()),
        "fetched" => Some(ctx.paths.fetched_dir.clone()),
        "folder" | "image" => source.path.as_ref().map(expand_home),
        _ => None,
    };

    let Some(path) = path else {
        return LocalSourceSummary {
            enabled: source.enabled,
            source_type: source.source_type.clone(),
            label,
            path: "(not configured)".into(),
            status: "missing path".into(),
            candidates: 0,
        };
    };

    let path_status = if path.exists() {
        "ready"
    } else {
        "missing path"
    };
    let enabled_status = if source.enabled { "" } else { "disabled, " };
    let candidates =
        list_images_with_paths(source, &ctx.paths.favorites_dir, &ctx.paths.fetched_dir)
            .map_or(0, |images| images.len());

    LocalSourceSummary {
        enabled: source.enabled,
        source_type: source.source_type.clone(),
        label,
        path: path.display().to_string(),
        status: format!("{enabled_status}{path_status}"),
        candidates,
    }
}

pub(super) fn summarize_config_warnings(ctx: &WallsCtx) -> Vec<String> {
    validate_config_diagnostics(&ctx.config, &ctx.secrets, &ctx.paths)
        .into_iter()
        .map(|diagnostic| {
            let mut warning = format!("warning: {}: {}", diagnostic.path, diagnostic.message);
            if let Some(hint) = diagnostic.hint {
                warning.push_str(&format!(" (hint: {hint})"));
            }
            warning
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::summarize_local_source;
    use walls_core::config::SourceEntry;
    use walls_core::WallsCtx;

    #[test]
    fn local_source_summary_reports_missing_unconfigured_path() {
        let root = tempfile::tempdir().expect("tempdir");
        let ctx = WallsCtx::load_from(root.path()).expect("ctx");
        let source = SourceEntry {
            enabled: true,
            source_type: "folder".into(),
            label: None,
            path: None,
            query: None,
            url: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            api_key: None,
            image_path: None,
            title_path: None,
            sort: None,
            time: None,
            ..SourceEntry::default()
        };

        let summary = summarize_local_source(&ctx, &source);

        assert!(summary.enabled);
        assert_eq!(summary.source_type, "folder");
        assert_eq!(summary.label, "folder");
        assert_eq!(summary.path, "(not configured)");
        assert_eq!(summary.status, "missing path");
        assert_eq!(summary.candidates, 0);
    }
}

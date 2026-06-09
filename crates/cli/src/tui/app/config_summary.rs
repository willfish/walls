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

pub struct WallhavenProviderSummary {
    pub enabled: bool,
    pub internet_enabled: bool,
    pub api_key_present: bool,
    pub prefer: String,
    pub collections: Vec<String>,
    pub query: String,
    pub categories: String,
    pub purity: String,
    pub sorting: String,
    pub order: String,
    pub atleast: String,
    pub warnings: Vec<String>,
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

pub(super) fn summarize_wallhaven_provider(ctx: &WallsCtx) -> WallhavenProviderSummary {
    let search = &ctx.config.wallhaven.search;
    let api_key_present = !ctx.secrets.wallhaven_api_key.trim().is_empty();
    let query = if search.q.trim().is_empty() {
        "(empty query)".into()
    } else {
        search.q.clone()
    };
    let collections = ctx
        .config
        .wallhaven
        .collections
        .iter()
        .map(|collection| {
            let label = collection.label.as_deref().unwrap_or("collection");
            format!("{}: {}/{}", label, collection.username, collection.id)
        })
        .collect();

    let mut warnings = Vec::new();
    if !ctx.config.change.internet_enabled {
        warnings.push("warning: online sources disabled".into());
    }
    if !api_key_present {
        warnings.push("warning: API key missing; NSFW purity unavailable".into());
    }
    if search.purity.chars().nth(2) == Some('1') {
        warnings.push("warning: NSFW purity requires Wallhaven account access".into());
    }

    WallhavenProviderSummary {
        enabled: ctx.config.wallhaven.enabled,
        internet_enabled: ctx.config.change.internet_enabled,
        api_key_present,
        prefer: format!("{:?}", ctx.config.wallhaven.prefer),
        collections,
        query,
        categories: search.categories.clone(),
        purity: search.purity.clone(),
        sorting: search.sorting.clone(),
        order: search.order.clone(),
        atleast: search.atleast.clone(),
        warnings,
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
    use super::{summarize_local_source, summarize_wallhaven_provider};
    use walls_core::config::{SourceEntry, WallhavenCollection};
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
        };

        let summary = summarize_local_source(&ctx, &source);

        assert!(summary.enabled);
        assert_eq!(summary.source_type, "folder");
        assert_eq!(summary.label, "folder");
        assert_eq!(summary.path, "(not configured)");
        assert_eq!(summary.status, "missing path");
        assert_eq!(summary.candidates, 0);
    }

    #[test]
    fn wallhaven_summary_includes_query_collections_and_warnings() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut ctx = WallsCtx::load_from(root.path()).expect("ctx");
        ctx.config.change.internet_enabled = false;
        ctx.config.wallhaven.search.q.clear();
        ctx.config.wallhaven.search.purity = "101".into();
        ctx.config.wallhaven.collections.push(WallhavenCollection {
            label: Some("space".into()),
            username: "ada".into(),
            id: 42,
        });

        let summary = summarize_wallhaven_provider(&ctx);

        assert_eq!(summary.query, "(empty query)");
        assert_eq!(summary.collections, ["space: ada/42"]);
        assert!(summary
            .warnings
            .contains(&"warning: online sources disabled".into()));
        assert!(summary
            .warnings
            .contains(&"warning: API key missing; NSFW purity unavailable".into()));
        assert!(summary
            .warnings
            .contains(&"warning: NSFW purity requires Wallhaven account access".into()));
    }
}

use walls_core::config::load_config;
use walls_core::config::SourceEntry;
use walls_core::config::TuiKeyProfile;
use walls_core::providers::{configured_source_providers, ProviderKind};

#[test]
fn loads_example_config() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("config.example.json");
    let cfg = load_config(&root).expect("example config should parse");
    assert!(cfg.change.enabled);
    assert_eq!(cfg.paths.cache_dir, "~/.local/share/walls/cache");
    assert_eq!(cfg.tui.key_profile, TuiKeyProfile::Emacs);

    // First-run defaults should be focused on sources that are immediately
    // useful, not a showcase list of every provider shape.
    let provs = configured_source_providers(&cfg.sources);
    let kinds: Vec<_> = provs.iter().map(|p| p.kind).collect();
    assert!(
        kinds.contains(&ProviderKind::Bing),
        "bing default should be in example for a working no-credential online source"
    );
    assert!(!kinds.contains(&ProviderKind::Reddit));
    assert!(!kinds.contains(&ProviderKind::Apod));
    assert!(!kinds.contains(&ProviderKind::MediaRss));
    assert!(!kinds.contains(&ProviderKind::Json));
}

#[test]
fn source_examples_cover_extended_provider_shapes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("config.sources.example.json");
    let data = std::fs::read_to_string(root).expect("source examples should be readable");
    let sources: Vec<SourceEntry> =
        serde_json::from_str(&data).expect("source examples should parse");

    let provs = configured_source_providers(&sources);
    let kinds: Vec<_> = provs.iter().map(|p| p.kind).collect();
    assert!(kinds.contains(&ProviderKind::Unsplash));
    assert!(kinds.contains(&ProviderKind::Reddit));
    assert!(kinds.contains(&ProviderKind::Apod));
    assert!(kinds.contains(&ProviderKind::MediaRss));
    assert!(kinds.contains(&ProviderKind::Json));
    assert!(kinds.contains(&ProviderKind::Pixabay));
    assert!(kinds.contains(&ProviderKind::Immich));
    assert!(kinds.contains(&ProviderKind::Spotlight));
}

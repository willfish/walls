use walls_core::config::load_config;
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
    assert_eq!(cfg.tui.key_profile, TuiKeyProfile::Default);

    // Feature test for defaults: the example must include working configs for all
    // providers so new users can get started. Prove via the public API that they
    // classify correctly (even if some disabled).
    let provs = configured_source_providers(&cfg.sources);
    let kinds: Vec<_> = provs.iter().map(|p| p.kind).collect();
    assert!(
        kinds.contains(&ProviderKind::Bing),
        "bing default should be in example for working start"
    );
    assert!(kinds.contains(&ProviderKind::Reddit));
    assert!(kinds.contains(&ProviderKind::Apod));
    assert!(kinds.contains(&ProviderKind::MediaRss));
    assert!(kinds.contains(&ProviderKind::Json));
    // etc for others; at least these to prove
}

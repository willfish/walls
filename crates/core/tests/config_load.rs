use walls_core::config::load_config;

#[test]
fn loads_example_config() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("config.example.json");
    let cfg = load_config(&root).expect("example config should parse");
    assert!(cfg.change.enabled);
    assert_eq!(cfg.paths.cache_dir, "~/.local/share/walls/cache");
}
use std::path::Path;

use walls_core::apply::{patch_cosmic_background, patch_wallpaper_path};
use walls_core::config::CosmicBackgroundEntryConfig;

#[test]
fn patch_replaces_source_path() {
    let input = r#"
backgrounds: (
    source: Path("/old/wall.jpg"),
    color: [0.0, 0.0, 0.0, 1.0],
)
"#;
    let out = patch_wallpaper_path(input, Path::new("/new/wall.jpg"));
    assert!(out.contains(r#"source: Path("/new/wall.jpg")"#));
    assert!(!out.contains("/old/wall.jpg"));
}

#[test]
fn patch_inserts_source_if_missing() {
    let input = r#"
backgrounds: (
    color: [0.0, 0.0, 0.0, 1.0],
)
"#;
    let out = patch_wallpaper_path(input, Path::new("/new/wall.jpg"));
    assert!(out.contains(r#"source: Path("/new/wall.jpg")"#));
    assert!(out.contains("color: [0.0, 0.0, 0.0, 1.0]"));
}

#[test]
fn patch_falls_back_to_minimal_if_no_backgrounds() {
    let input = r#"
color: [0.0, 0.0, 0.0, 1.0],
"#;
    let out = patch_wallpaper_path(input, Path::new("/new/wall.jpg"));
    assert!(out.contains(r#"source: Path("/new/wall.jpg")"#));
}

#[test]
fn patch_works_on_real_user_cosmic_ron_if_present() {
    // Exercises the patch on the *actual* ron from the user's env (the one that needed the native
    // switcher before walls n/p would cause visible changes). If present, we verify that we can
    // successfully inject/update a source path (as would happen on TUI 'n'/'p' for cosmic backend).
    // This is "actually tried" with the user's real cosmic config state.
    let p =
        std::path::Path::new("/home/william/.config/cosmic/com.system76.CosmicBackground/v1/all");
    if !p.is_file() {
        eprintln!("note: no real cosmic ron at {p:?} in this env; skipping live-ron patch test");
        return;
    }
    let contents = std::fs::read_to_string(p).expect("read real ron");
    let test_wall = std::path::Path::new("/tmp/walls-actual-n-test-wallpaper.jpg");
    let out = patch_wallpaper_path(&contents, test_wall);
    assert!(
        out.contains(r#"source: Path("/tmp/walls-actual-n-test-wallpaper.jpg")"#),
        "should be able to patch a source into the user's real ron (post or pre switcher); result snippet: {}",
        &out[..out.len().min(300)]
    );
}

#[test]
fn patch_never_leaves_duplicate_source_on_flat_cosmic_all_file() {
    // Real on-disk shape for ~/.config/cosmic/.../v1/all (output: "all" tuple, not backgrounds: (...)).
    let clean = r#"(
    output: "all",
    filter_by_theme: true,
    rotation_frequency: 300,
    filter_method: Lanczos,
    scaling_mode: Zoom,
    sampling_method: Alphanumeric,
)
"#;
    let once = patch_wallpaper_path(clean, Path::new("/first.jpg"));
    assert_eq!(
        once.matches("source: Path").count(),
        1,
        "first patch should add exactly one source; got:\n{once}"
    );
    let twice = patch_wallpaper_path(&once, Path::new("/second.jpg"));
    assert_eq!(
        twice.matches("source: Path").count(),
        1,
        "second patch must not duplicate source; got:\n{twice}"
    );
    assert!(twice.contains("/second.jpg"));
    assert!(!twice.contains("/first.jpg"));
    assert!(
        twice.contains("rotation_frequency: 0"),
        "walls should disable COSMIC slideshow on apply; got:\n{twice}"
    );
    assert!(
        twice.contains("filter_by_theme: false"),
        "walls should pin explicit file path mode; got:\n{twice}"
    );
}

#[test]
fn patch_replaces_misconfigured_cosmic_slideshow_settings() {
    let misconfigured = r#"(
    output: "all",
    source: Path("/old.jpg"),
    filter_by_theme: true,
    rotation_frequency: 300,
    filter_method: Lanczos,
    scaling_mode: Zoom,
    sampling_method: Alphanumeric,
)
"#;
    let out = patch_wallpaper_path(misconfigured, Path::new("/new.jpg"));
    assert!(out.contains(r#"source: Path("/new.jpg")"#));
    assert!(out.contains("rotation_frequency: 0"));
    assert!(out.contains("filter_by_theme: false"));
    assert!(!out.contains("rotation_frequency: 300"));
}

#[test]
fn applier_creates_missing_cosmic_config_with_managed_defaults() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_path = tmp.path().join("all");
    let applier =
        walls_core::apply::CosmicConfigApplier::new(&walls_core::config::CosmicApplyConfig {
            config_path: config_path.display().to_string(),
            ..Default::default()
        });
    applier
        .apply_path(Path::new("/tmp/walls-created.jpg"))
        .expect("apply to missing config");
    let written = std::fs::read_to_string(&config_path).expect("config written");
    assert!(written.contains(r#"source: Path("/tmp/walls-created.jpg")"#));
    assert!(written.contains("rotation_frequency: 0"));
    assert!(written.contains("filter_by_theme: false"));
}

#[test]
fn patch_respects_custom_entry_policy_from_config() {
    let input = r#"(
    output: "all",
    rotation_frequency: 300,
    filter_by_theme: true,
)
"#;
    let entry = CosmicBackgroundEntryConfig {
        rotation_frequency: 900,
        filter_by_theme: true,
    };
    let out = patch_cosmic_background(input, Path::new("/custom.jpg"), &entry);
    assert!(out.contains("rotation_frequency: 900"));
    assert!(out.contains("filter_by_theme: true"));
}

#[test]
fn patch_replaces_cosmic_spaced_path_variant_without_duplicating() {
    // COSMIC's RON serializer may emit `source: Path ("/path")` (space before `(`).
    // The old regex-only replace missed that and injected a second source.
    let input = r#"(
    output: "all",
    source: Path ("/cosmic-native.jpg"),
    filter_by_theme: true,
)
"#;
    let out = patch_wallpaper_path(input, Path::new("/walls.jpg"));
    assert_eq!(
        out.matches("source: Path").count(),
        1,
        "must not leave a stale spaced Path entry; got:\n{out}"
    );
    assert!(out.contains(r#"source: Path("/walls.jpg")"#));
    assert!(!out.contains("cosmic-native"));
}

#[test]
fn patch_dedupes_existing_duplicate_sources() {
    let corrupted = r#"(
    output: "all",
    source: Path("/dup.jpg"), source: Path("/dup.jpg"),
    filter_by_theme: true,
)
"#;
    let out = patch_wallpaper_path(corrupted, Path::new("/fixed.jpg"));
    assert_eq!(
        out.matches("source: Path").count(),
        1,
        "patch should collapse duplicate sources; got:\n{out}"
    );
    assert!(out.contains("/fixed.jpg"));
}

#[test]
fn patch_handles_nested_output_structure_and_injects_source_inside_output_tuple() {
    // Realistic structure after COSMIC switcher or multi-output setup.
    // Pre-switcher or missing-source files may look like this (no source yet).
    // The patch must result in a source *inside* the output config so that the DE honors it for that output.
    // (Current simple insert at outer backgrounds:( would put it at wrong level.)
    let input = r#"
backgrounds: (
	(
		output: "eDP-1",
		color: [0.0, 0.0, 0.0, 1.0],
	),
)
"#;
    let out = patch_wallpaper_path(input, Path::new("/new/wall.jpg"));
    assert!(out.contains(r#"source: Path("/new/wall.jpg")"#));
    // Must be associated with the output (source textually after the output: line, as if inserted inside that tuple).
    // Current outer-insert puts source before the inner ( output, so this will fail until we improve patch.
    let output_pos = out.find("output: \"eDP-1\"").expect("has output");
    let source_pos = out
        .find(r#"source: Path("/new/wall.jpg")"#)
        .expect("has source");
    assert!(
        source_pos > output_pos,
        "source should appear after the output line (inside the tuple) for proper structure; source@{} output@{}; got:\n{}",
        source_pos, output_pos, out
    );
    // Original content preserved.
    assert!(out.contains("color: [0.0, 0.0, 0.0, 1.0]"));
}

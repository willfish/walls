use std::path::Path;

use walls_core::apply::patch_wallpaper_path;

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

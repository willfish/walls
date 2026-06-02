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

use std::fs;

use walls_core::config::SourceEntry;
use walls_core::list_images;

#[test]
fn lists_images_in_folder() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.jpg"), b"x").unwrap();
    fs::write(dir.path().join("b.png"), b"x").unwrap();
    fs::write(dir.path().join("skip.txt"), b"x").unwrap();

    let src = SourceEntry {
        enabled: true,
        source_type: "folder".into(),
        label: None,
        path: Some(dir.path().display().to_string()),
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
    let images = list_images(&src).unwrap();
    assert_eq!(images.len(), 2);
}

use std::fs;

use walls_core::wallhaven::SearchResponse;

#[test]
fn parses_search_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wallhaven-search.json");
    let json = fs::read_to_string(path).unwrap();
    let resp: SearchResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "94x38z");
    assert_eq!(
        resp.data[0].path,
        "https://w.wallhaven.cc/full/94/wallhaven-94x38z.jpg"
    );
    assert_eq!(resp.meta.current_page, 1);
    assert_eq!(resp.meta.last_page, 36);
}
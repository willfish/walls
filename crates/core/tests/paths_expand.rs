use walls_core::expand_home;

#[test]
fn expand_tilde() {
    let p = expand_home("~/Pictures/wall.jpg");
    assert!(p.ends_with("Pictures/wall.jpg"));
    assert!(!p.to_string_lossy().contains('~'));
}

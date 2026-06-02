use walls_core::selection::{pick_next, PickInput};

#[test]
fn avoids_recent_ids() {
    let candidates = vec!["a".into(), "b".into(), "c".into()];
    let recent = vec!["a".into(), "b".into()];
    let pick = pick_next(&PickInput {
        candidates: &candidates,
        recent: &recent,
        avoid_recent: 10,
    })
    .unwrap();
    assert_eq!(pick, "c");
}
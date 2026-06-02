use walls_core::state::State;

#[test]
fn state_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let state = State {
        paused: true,
        history: vec!["abc".into()],
        ..Default::default()
    };
    state.save(&path).unwrap();
    let loaded = State::load_or_default(&path).unwrap();
    assert!(loaded.paused);
    assert_eq!(loaded.history, vec!["abc"]);
}

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use walls_core::lock::StateLock;
use walls_core::state::State;

#[test]
fn second_lock_waits_for_first_to_release() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    fs::write(&state_file, "{}").unwrap();

    let lock1 = StateLock::acquire(&state_file).unwrap();
    let path = state_file.clone();
    let ready = Arc::new(Barrier::new(2));
    let ready2 = Arc::clone(&ready);

    let handle = thread::spawn(move || {
        ready2.wait();
        let start = Instant::now();
        let _lock2 = StateLock::acquire(&path).unwrap();
        start.elapsed()
    });

    ready.wait();
    thread::sleep(Duration::from_millis(150));
    assert!(!handle.is_finished());
    drop(lock1);
    let waited = handle.join().unwrap();
    assert!(waited >= Duration::from_millis(20));
}

#[test]
fn second_lock_waits_after_state_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    fs::write(&state_file, "{}").unwrap();

    let lock1 = StateLock::acquire(&state_file).unwrap();
    State {
        paused: true,
        ..State::default()
    }
    .save(&state_file)
    .unwrap();

    let path = state_file.clone();
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let _lock2 = StateLock::acquire(&path).unwrap();
        start.elapsed()
    });

    thread::sleep(Duration::from_millis(150));
    assert!(!handle.is_finished());
    drop(lock1);
    let waited = handle.join().unwrap();
    assert!(waited >= Duration::from_millis(20));
}

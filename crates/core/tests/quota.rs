use std::fs::File;
use std::io::Write;
use std::time::{Duration, UNIX_EPOCH};

use walls_core::quota::enforce_download_quota_bytes;

#[test]
fn enforce_download_quota_deletes_oldest_when_over_limit() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.jpg");
    let new = dir.path().join("new.jpg");

    write_file(&old, 600, 1);
    write_file(&new, 600, 2);

    // 1024-byte cap: two 600-byte files exceed it; oldest (old.jpg) removed.
    enforce_download_quota_bytes(dir.path(), 1024).unwrap();

    assert!(!old.exists());
    assert!(new.exists());
}

#[test]
fn enforce_download_quota_deletes_only_enough_oldest_files() {
    let dir = tempfile::tempdir().unwrap();
    let oldest = dir.path().join("oldest.jpg");
    let older = dir.path().join("older.jpg");
    let newer = dir.path().join("newer.jpg");
    let newest = dir.path().join("newest.jpg");

    write_file(&oldest, 300, 1);
    write_file(&older, 400, 2);
    write_file(&newer, 500, 3);
    write_file(&newest, 600, 4);

    // 1800 bytes total with an 1100-byte cap requires deleting the two oldest
    // files, then stopping as soon as the remaining total reaches the cap.
    enforce_download_quota_bytes(dir.path(), 1100).unwrap();

    assert!(!oldest.exists());
    assert!(!older.exists());
    assert!(newer.exists());
    assert!(newest.exists());
}

fn write_file(path: &std::path::Path, size: usize, modified_secs: u64) {
    let mut file = File::create(path).unwrap();
    file.write_all(&vec![0u8; size]).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::from_secs(modified_secs))
        .unwrap();
}

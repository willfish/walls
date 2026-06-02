use std::fs::File;
use std::io::Write;
use std::time::{Duration, UNIX_EPOCH};

use walls_core::quota::enforce_download_quota_bytes;

#[test]
fn enforce_download_quota_deletes_oldest_when_over_limit() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.jpg");
    let new = dir.path().join("new.jpg");

    let mut f = File::create(&old).unwrap();
    f.write_all(&vec![0u8; 600]).unwrap();
    f.set_modified(UNIX_EPOCH + Duration::from_secs(1)).unwrap();
    drop(f);

    let mut f = File::create(&new).unwrap();
    f.write_all(&vec![0u8; 600]).unwrap();
    f.set_modified(UNIX_EPOCH + Duration::from_secs(2)).unwrap();
    drop(f);

    // 1024-byte cap — two 600-byte files exceed it; oldest (old.jpg) removed
    enforce_download_quota_bytes(dir.path(), 1024).unwrap();

    assert!(!old.exists());
    assert!(new.exists());
}
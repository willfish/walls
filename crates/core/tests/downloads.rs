use std::fs;
use walls_core::downloads::{
    copy_file_atomic, inspect_cache, is_provider_cache_file_name, list_cache_files, nuke_downloads,
    plan_nuke_downloads, write_file_atomic, NukeDownloadsMode,
};
use walls_core::paths::WallsPaths;
use walls_core::state::State;

fn temp_paths(root: &std::path::Path) -> WallsPaths {
    WallsPaths {
        config_dir: root.join("config"),
        config_file: root.join("config.json"),
        secrets_file: root.join("secrets.json"),
        state_file: root.join("state.json"),
        cache_dir: root.join("cache"),
        download_dir: root.join("downloaded"),
        favorites_dir: root.join("favorites"),
        fetched_dir: root.join("fetched"),
        compose_dir: root.join("compose"),
    }
}

#[test]
fn provider_cache_file_name_detection() {
    assert!(is_provider_cache_file_name("wallhaven-abc123.jpg"));
    assert!(is_provider_cache_file_name("unsplash-photo.webp"));
    assert!(is_provider_cache_file_name("bing-daily.jpg"));
    assert!(!is_provider_cache_file_name("my-folder-shot.png"));
    assert!(!is_provider_cache_file_name("favorite-copy.jpg"));
}

#[tokio::test]
async fn write_file_atomic_writes_final_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dest = tmp.path().join("cache").join("wallhaven-abc.jpg");

    write_file_atomic(&dest, b"complete")
        .await
        .expect("atomic write");

    assert_eq!(fs::read(&dest).expect("read final"), b"complete");
    assert_no_temp_files(tmp.path());
}

#[tokio::test]
async fn write_file_atomic_does_not_replace_existing_directory_or_leave_temp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dest = tmp.path().join("cache").join("wallhaven-abc.jpg");
    fs::create_dir_all(&dest).expect("directory at final path");

    let err = write_file_atomic(&dest, b"complete")
        .await
        .expect_err("rename over directory should fail");

    assert!(err.to_string().contains("Is a directory") || err.to_string().contains("directory"));
    assert!(dest.is_dir(), "existing final directory should remain");
    assert_no_temp_files(tmp.path());
}

#[tokio::test]
async fn copy_file_atomic_writes_final_copy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = tmp.path().join("source.jpg");
    let dest = tmp.path().join("downloaded").join("source.jpg");
    fs::write(&src, b"cached").expect("source");

    copy_file_atomic(&src, &dest).await.expect("atomic copy");

    assert_eq!(fs::read(&dest).expect("read copied"), b"cached");
    assert_no_temp_files(tmp.path());
}

#[test]
fn nuke_clears_queue_before_purging_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = temp_paths(tmp.path());
    fs::create_dir_all(&paths.cache_dir).expect("cache");
    fs::create_dir_all(&paths.download_dir).expect("download");
    fs::write(paths.cache_dir.join("wallhaven-abc.jpg"), b"x").expect("cache file");
    fs::write(paths.download_dir.join("copy.jpg"), b"x").expect("download file");

    let mut state = State {
        cache_queue: vec!["abc".into(), "def".into()],
        ..State::default()
    };

    let plan = plan_nuke_downloads(&paths, &state);
    assert_eq!(plan.mode, NukeDownloadsMode::ClearQueue);
    assert_eq!(plan.queue_len, 2);

    let result = nuke_downloads(&paths, &mut state).expect("nuke");
    assert_eq!(result.mode, NukeDownloadsMode::ClearQueue);
    assert_eq!(result.queue_cleared, 2);
    assert!(state.cache_queue.is_empty());
    assert!(paths.cache_dir.join("wallhaven-abc.jpg").exists());
    assert!(paths.download_dir.join("copy.jpg").exists());
}

#[test]
fn nuke_purges_provider_files_when_queue_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = temp_paths(tmp.path());
    fs::create_dir_all(&paths.cache_dir).expect("cache");
    fs::create_dir_all(&paths.download_dir).expect("download");
    fs::create_dir_all(&paths.fetched_dir).expect("fetched");
    fs::write(paths.cache_dir.join("wallhaven-abc.jpg"), b"x").expect("cache file");
    fs::write(paths.cache_dir.join("local-import.jpg"), b"x").expect("local cache");
    fs::write(paths.download_dir.join("copy.jpg"), b"x").expect("download file");
    fs::write(paths.fetched_dir.join("imported.jpg"), b"x").expect("fetched file");

    let mut state = State::default();
    state.history.push(
        paths
            .cache_dir
            .join("wallhaven-abc.jpg")
            .display()
            .to_string(),
    );
    state
        .history
        .push(paths.fetched_dir.join("imported.jpg").display().to_string());
    state.current = Some(walls_core::state::CurrentWall {
        source_id: "wallhaven-abc.jpg".into(),
        wallhaven_id: Some("abc".into()),
        provider: Some("wallhaven".into()),
        source_url: None,
        author: None,
        description: None,
        original_path: paths
            .cache_dir
            .join("wallhaven-abc.jpg")
            .display()
            .to_string(),
        composed_path: paths.compose_dir.join("composed.jpg").display().to_string(),
        post_filter_path: None,
    });

    let result = nuke_downloads(&paths, &mut state).expect("nuke");
    assert_eq!(result.mode, NukeDownloadsMode::PurgeProviderFiles);
    assert_eq!(result.cache_removed, 1);
    assert_eq!(result.download_removed, 1);
    assert!(!paths.cache_dir.join("wallhaven-abc.jpg").exists());
    assert!(paths.cache_dir.join("local-import.jpg").exists());
    assert!(!paths.download_dir.join("copy.jpg").exists());
    assert!(paths.fetched_dir.join("imported.jpg").exists());
    assert!(state.current.is_none());
    assert_eq!(state.history.len(), 1);
    assert!(state.history[0].ends_with("imported.jpg"));
}

#[test]
fn inspect_cache_reports_sizes_queue_and_provider_references() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = temp_paths(tmp.path());
    fs::create_dir_all(&paths.cache_dir).expect("cache");
    fs::create_dir_all(&paths.download_dir).expect("download");
    fs::write(paths.cache_dir.join("wallhaven-abc.jpg"), b"abc").expect("cache file");
    fs::write(paths.cache_dir.join("local-import.jpg"), b"local").expect("local file");
    fs::write(paths.download_dir.join("unsplash-def.jpg"), b"data").expect("download file");

    let state = State {
        cache_queue: vec!["wallhaven:abc".into()],
        history: vec![
            paths
                .cache_dir
                .join("wallhaven-abc.jpg")
                .display()
                .to_string(),
            tmp.path().join("local.jpg").display().to_string(),
        ],
        current: Some(walls_core::state::CurrentWall {
            source_id: "wallhaven-abc.jpg".into(),
            wallhaven_id: Some("abc".into()),
            provider: Some("wallhaven".into()),
            source_url: None,
            author: None,
            description: None,
            original_path: paths
                .cache_dir
                .join("wallhaven-abc.jpg")
                .display()
                .to_string(),
            composed_path: paths.compose_dir.join("composed.jpg").display().to_string(),
            post_filter_path: None,
        }),
        ..State::default()
    };

    let inspection = inspect_cache(&paths, &state);

    assert_eq!(inspection.queue_len, 1);
    assert_eq!(inspection.queue_ids, vec!["wallhaven:abc"]);
    assert_eq!(inspection.cache.files, 2);
    assert_eq!(inspection.cache.bytes, 8);
    assert_eq!(inspection.cache.provider_files, 1);
    assert_eq!(inspection.cache.provider_bytes, 3);
    assert_eq!(inspection.downloads.files, 1);
    assert_eq!(inspection.downloads.bytes, 4);
    assert!(inspection.current_provider_storage);
    assert_eq!(inspection.history_provider_entries, 1);
}

#[test]
fn list_cache_files_filters_provider_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = temp_paths(tmp.path());
    fs::create_dir_all(&paths.cache_dir).expect("cache");
    fs::create_dir_all(&paths.download_dir).expect("download");
    fs::write(paths.cache_dir.join("wallhaven-abc.jpg"), b"abc").expect("wallhaven");
    fs::write(paths.cache_dir.join("unsplash-def.jpg"), b"def").expect("unsplash");
    fs::write(paths.cache_dir.join("local-import.jpg"), b"local").expect("local");
    fs::write(paths.download_dir.join("wallhaven-copy.jpg"), b"copy").expect("download");

    let files = list_cache_files(&paths, Some("wallhaven"));

    assert_eq!(files.len(), 2);
    assert!(files
        .iter()
        .all(|file| file.provider.as_deref() == Some("wallhaven")));
    assert!(files.iter().any(|file| file.name == "wallhaven-abc.jpg"));
    assert!(files.iter().any(|file| file.name == "wallhaven-copy.jpg"));
}

fn assert_no_temp_files(root: &std::path::Path) {
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        assert!(!name.contains(".tmp-"), "unexpected temp file: {name}");
    }
}

use std::fs;
use std::hint::black_box;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use walls_core::config::SourceEntry;
use walls_core::providers::{configured_source_providers, enabled_local_sources};
use walls_core::wallhaven::cached_wallpaper_path;
use walls_core::{enforce_download_quota_bytes, list_images, pick_next, PickInput};

fn bench_selection(c: &mut Criterion) {
    let candidates: Vec<String> = (0..1_024).map(|i| format!("wallpaper-{i:04}")).collect();
    let recent: Vec<String> = candidates.iter().take(256).cloned().collect();

    c.bench_function("selection/pick_next_1024_candidates_256_recent", |b| {
        b.iter(|| {
            pick_next(black_box(&PickInput {
                candidates: &candidates,
                recent: &recent,
                avoid_recent: 256,
            }))
            .expect("candidate selected")
        });
    });
}

fn bench_local_source_listing(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..128 {
        fs::write(dir.path().join(format!("wallpaper-{i:03}.jpg")), b"x").expect("image");
        fs::write(dir.path().join(format!("notes-{i:03}.txt")), b"x").expect("non-image");
    }
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("nested dir");
    for i in 0..32 {
        fs::write(nested.join(format!("nested-{i:03}.png")), b"x").expect("nested image");
    }
    let source = source_entry(
        "folder",
        Some(dir.path().to_string_lossy().into_owned()),
        true,
    );

    c.bench_function("local_source/list_images_160_images", |b| {
        b.iter(|| list_images(black_box(&source)).expect("listed images"));
    });
}

fn bench_quota(c: &mut Criterion) {
    c.bench_function("quota/enforce_64_files_delete_half", |b| {
        b.iter_batched(
            quota_fixture,
            |dir| {
                enforce_download_quota_bytes(black_box(dir.path()), black_box(32 * 1024))
                    .expect("quota enforced");
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_wallhaven_cache(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..512 {
        fs::write(dir.path().join(format!("wallhaven-fill-{i:03}.jpg")), b"x").expect("filler");
    }
    fs::write(dir.path().join("wallhaven-hit.jpg"), b"x").expect("standard hit");
    fs::write(dir.path().join("wallhaven-legacy.gif"), b"x").expect("legacy hit");

    c.bench_function("wallhaven_cache/standard_extension_hit", |b| {
        b.iter(|| cached_wallpaper_path(black_box(dir.path()), black_box("hit")));
    });
    c.bench_function("wallhaven_cache/legacy_extension_scan", |b| {
        b.iter(|| cached_wallpaper_path(black_box(dir.path()), black_box("legacy")));
    });
    c.bench_function("wallhaven_cache/missing_id_scan", |b| {
        b.iter(|| cached_wallpaper_path(black_box(dir.path()), black_box("missing")));
    });
}

fn bench_source_dispatch(c: &mut Criterion) {
    let sources = dispatch_sources();

    c.bench_function("source_dispatch/configured_source_providers_640", |b| {
        b.iter(|| configured_source_providers(black_box(&sources)));
    });
    c.bench_function("source_dispatch/enabled_local_sources_640", |b| {
        b.iter(|| enabled_local_sources(black_box(&sources)).count());
    });
}

fn quota_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..64 {
        write_file(
            &dir.path().join(format!("wallpaper-{i:03}.jpg")),
            1024,
            i + 1,
        );
    }
    dir
}

fn write_file(path: &Path, size: usize, modified_secs: u64) {
    let mut file = fs::File::create(path).expect("file");
    file.write_all(&vec![0u8; size]).expect("file contents");
    file.set_modified(UNIX_EPOCH + Duration::from_secs(modified_secs))
        .expect("modified timestamp");
}

fn dispatch_sources() -> Vec<SourceEntry> {
    let types = [
        "folder",
        "favorites",
        "fetched",
        "image",
        "unsplash",
        "reddit",
        "bing",
        "apod",
        "mediarss",
        "attribution",
        "json",
        "pixabay",
        "immich",
        "spotlight",
        "weighting",
        "future-provider",
    ];

    (0..640)
        .map(|i| {
            source_entry(
                types[i % types.len()],
                Some(format!("/tmp/walls/{i}")),
                i % 3 != 0,
            )
        })
        .collect()
}

fn source_entry(source_type: &str, path: Option<String>, enabled: bool) -> SourceEntry {
    SourceEntry {
        enabled,
        source_type: source_type.into(),
        label: Some(format!("{source_type}-fixture")),
        path,
        query: Some("forest".into()),
        url: Some("https://example.com/feed.json".into()),
        collection: Some("collection".into()),
        user: Some("user".into()),
        topic: Some("topic".into()),
        orientation: Some("landscape".into()),
        api_key: Some("key".into()),
        image_path: Some("$.image".into()),
        title_path: Some("$.title".into()),
        sort: Some("top".into()),
        time: Some("month".into()),
        ..SourceEntry::default()
    }
}

criterion_group!(
    benches,
    bench_selection,
    bench_local_source_listing,
    bench_quota,
    bench_wallhaven_cache,
    bench_source_dispatch
);
criterion_main!(benches);

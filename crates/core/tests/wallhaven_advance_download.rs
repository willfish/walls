mod common {
    #![allow(dead_code)]
    include!("common/harness.rs");
    include!("common/wallhaven_mock.rs");
}

use common::FetchHarness;
use wiremock::MockServer;

#[tokio::test]
async fn advance_next_downloads_when_queued_but_not_cached() {
    let server = MockServer::start().await;
    let image_url = common::wallhaven_image_path(&server.uri());
    let _api_base = common::lock_wallhaven_api_base(&server.uri());
    common::mount_wallhaven_wallpaper_mock(&server, "key", &image_url).await;
    common::mount_wallhaven_image_mock(&server).await;

    let harness = FetchHarness::new();
    harness.write_config(
        harness.wallhaven_only_config(FetchHarness::wallhaven_provider(serde_json::json!({}))),
    );
    harness.write_secrets(FetchHarness::wallhaven_secrets("key"));
    harness.write_state(serde_json::json!({
        "cache_queue": [common::WALLHAVEN_FIXTURE_ID],
    }));

    let mut ctx = harness.load_ctx();
    let applied = ctx.advance_next().await.unwrap().expect("applied");
    assert!(applied.exists());
    assert!(ctx.state.cache_queue.is_empty());
}

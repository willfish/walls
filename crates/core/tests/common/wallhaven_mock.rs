// Wiremock fixtures for hermetic Wallhaven e2e tests.

use std::sync::{Mutex, MutexGuard};

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub const WALLHAVEN_FIXTURE_ID: &str = "94x38z";

static WALLHAVEN_API_BASE_LOCK: Mutex<()> = Mutex::new(());

/// Hold for the whole test — `WALLHAVEN_API_BASE` is process-global.
pub struct WallhavenApiBaseGuard {
    _lock: MutexGuard<'static, ()>,
}

pub fn wallhaven_image_path(server_uri: &str) -> String {
    format!("{server_uri}/wallhaven-{WALLHAVEN_FIXTURE_ID}.jpg")
}

pub fn lock_wallhaven_api_base(server_uri: &str) -> WallhavenApiBaseGuard {
    let lock = WALLHAVEN_API_BASE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    std::env::set_var("WALLHAVEN_API_BASE", server_uri);
    WallhavenApiBaseGuard { _lock: lock }
}

pub async fn mount_wallhaven_search_mock(server: &MockServer, api_key: &str) {
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", api_key))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("../fixtures/wallhaven-search.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

pub async fn mount_wallhaven_wallpaper_mock(
    server: &MockServer,
    api_key: &str,
    image_url: &str,
) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/w/{WALLHAVEN_FIXTURE_ID}")))
        .and(header("X-API-Key", api_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "id": WALLHAVEN_FIXTURE_ID, "path": image_url }
        })))
        .mount(server)
        .await;
}

pub async fn mount_wallhaven_image_mock(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/wallhaven-{WALLHAVEN_FIXTURE_ID}.jpg")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wallhaven-jpeg"))
        .mount(server)
        .await;
}

pub async fn mount_wallhaven_fetch_flow(server: &MockServer, api_key: &str) -> WallhavenApiBaseGuard {
    let image_url = wallhaven_image_path(&server.uri());
    mount_wallhaven_search_mock(server, api_key).await;
    mount_wallhaven_wallpaper_mock(server, api_key, &image_url).await;
    mount_wallhaven_image_mock(server).await;
    lock_wallhaven_api_base(&server.uri())
}

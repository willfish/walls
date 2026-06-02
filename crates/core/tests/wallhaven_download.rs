use std::fs;

use walls_core::wallhaven::{WallhavenClient, Wallpaper};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn download_to_cache_writes_file() {
    let server = MockServer::start().await;
    let image_url = format!("{}/wallpaper.jpg", server.uri());
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/wallpaper.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jpeg-bytes"))
        .mount(&server)
        .await;

    let wp = Wallpaper {
        id: "abc123".into(),
        path: image_url,
    };
    let cache = tempfile::tempdir().unwrap();
    let client = WallhavenClient::new(server.uri(), "").unwrap();
    let dest = client.download_to_cache(&wp, cache.path()).await.unwrap();

    assert!(dest.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"jpeg-bytes");
    assert!(dest
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .contains("wallhaven-abc123"));
}

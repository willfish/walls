use std::fs;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use walls_core::wallhaven::{WallhavenClient, Wallpaper};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn wallhaven_download_to_cache_writes_file() {
    let server = MockServer::start().await;
    let image_url = format!("{}/wallpaper.jpg", server.uri());
    Mock::given(method("GET"))
        .and(path("/wallpaper.jpg"))
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

#[tokio::test]
async fn wallhaven_download_to_cache_rejects_over_limit_content_length() {
    let server = MockServer::start().await;
    let image_url = format!("{}/wallpaper.jpg", server.uri());
    Mock::given(method("GET"))
        .and(path("/wallpaper.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", "11")
                .set_body_bytes(b"hello world"),
        )
        .mount(&server)
        .await;

    let wp = Wallpaper {
        id: "abc123".into(),
        path: image_url,
    };
    let cache = tempfile::tempdir().unwrap();
    let client = client_with_download_limit(server.uri(), 10);
    let err = client
        .download_to_cache(&wp, cache.path())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("exceeds limit of 10 bytes"));
    assert!(!cache.path().join("wallhaven-abc123.jpg").exists());
}

#[tokio::test]
async fn wallhaven_download_to_cache_rejects_chunked_response_once_limit_is_exceeded() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\n\
                  Transfer-Encoding: chunked\r\n\
                  Content-Type: image/jpeg\r\n\
                  \r\n\
                  6\r\nabcdef\r\n\
                  6\r\nghijkl\r\n\
                  0\r\n\r\n",
            )
            .await
            .unwrap();
    });

    let wp = Wallpaper {
        id: "chunked".into(),
        path: format!("http://{addr}/wallpaper.jpg"),
    };
    let cache = tempfile::tempdir().unwrap();
    let client = client_with_download_limit(format!("http://{addr}"), 10);
    let err = client
        .download_to_cache(&wp, cache.path())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("exceeded limit of 10 bytes"));
    assert!(!cache.path().join("wallhaven-chunked.jpg").exists());
}

fn client_with_download_limit(base_url: String, max_download_bytes: u64) -> WallhavenClient {
    WallhavenClient::new_with_limits(
        base_url,
        "",
        Duration::from_secs(30),
        Duration::from_secs(10),
        max_download_bytes,
    )
    .unwrap()
}

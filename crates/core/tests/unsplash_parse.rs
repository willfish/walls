use walls_core::config::{SourceEntry, UnsplashSourceConfig};
use walls_core::unsplash::Photo;

#[test]
fn parses_photo_metadata_fixture() {
    let photo: Photo =
        serde_json::from_str(include_str!("fixtures/unsplash-photo.json")).expect("photo");

    assert_eq!(photo.id, "abc123");
    assert_eq!(
        photo.urls.wallpaper_url(),
        "https://images.unsplash.com/photo-abc123.jpg"
    );
    assert_eq!(photo.links.html, "https://unsplash.com/photos/abc123");
    assert_eq!(photo.best_description(), Some("A mountain at sunrise"));
    assert_eq!(photo.user.name, "Ada Lovelace");
}

#[test]
fn parses_supported_unsplash_source_urls() {
    let collection = config_from_url("https://unsplash.com/collections/12345/wallpapers");
    assert_eq!(collection.collection.as_deref(), Some("12345"));

    let user = config_from_url("https://unsplash.com/example-user");
    assert_eq!(user.user.as_deref(), Some("example-user"));

    let topic = config_from_url("https://unsplash.com/t/nature");
    assert_eq!(topic.topic.as_deref(), Some("nature"));

    let query = config_from_url("https://unsplash.com/s/photos/night-sky");
    assert_eq!(query.query.as_deref(), Some("night sky"));
}

#[test]
fn rejects_collection_query_combination() {
    let source = source_with_url("https://unsplash.com/collections/12345/wallpapers")
        .with_query("mountains");

    let error = UnsplashSourceConfig::from_source(&source).expect_err("invalid source");

    assert!(
        error.to_string().contains("cannot also set query"),
        "{error}"
    );
}

fn config_from_url(url: &str) -> UnsplashSourceConfig {
    UnsplashSourceConfig::from_source(&source_with_url(url)).expect("source config")
}

fn source_with_url(url: &str) -> TestSourceBuilder {
    TestSourceBuilder {
        source: SourceEntry {
            enabled: true,
            source_type: "unsplash".into(),
            label: None,
            path: None,
            query: None,
            url: Some(url.into()),
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            api_key: None,
            image_path: None,
            title_path: None,
            sort: None,
            time: None,
            ..SourceEntry::default()
        },
    }
}

struct TestSourceBuilder {
    source: SourceEntry,
}

impl TestSourceBuilder {
    fn with_query(mut self, query: &str) -> SourceEntry {
        self.source.query = Some(query.into());
        self.source
    }
}

impl std::ops::Deref for TestSourceBuilder {
    type Target = SourceEntry;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

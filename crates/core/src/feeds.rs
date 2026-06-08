//! JSON-path and RSS extractors used by inline feed providers (json, mediarss).

/// Resolve a dotted JSON path such as `$.download_url` or `$.images[0].url`.
pub fn extract_json_string(value: &serde_json::Value, path: &str) -> Option<String> {
    if !path.starts_with("$.") {
        return value
            .get(path)
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
    }
    let mut current = value;
    for part in path[2..].split('.') {
        let part = part.trim();
        if !part.is_empty() {
            if let Some(idx_start) = part.find('[') {
                let key = &part[..idx_start];
                if !key.is_empty() {
                    current = current.get(key)?;
                }
                if let Some(end) = part.find(']') {
                    if let Ok(i) = part[idx_start + 1..end].parse::<usize>() {
                        current = current.get(i)?;
                    }
                }
            } else {
                current = current.get(part)?;
            }
        }
    }
    current.as_str().map(ToString::to_string)
}

/// First image URL from a Media RSS / enclosure feed.
pub fn extract_first_media_from_rss(xml: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?:enclosure|media:content)[^>]*url=["']([^"']+)["']"#).ok()?;
    re.captures(xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_string_resolves_dotted_path() {
        let value = serde_json::json!({
            "download_url": "https://example.com/a.jpg",
            "nested": { "url": "https://example.com/b.jpg" }
        });
        assert_eq!(
            extract_json_string(&value, "$.download_url").as_deref(),
            Some("https://example.com/a.jpg")
        );
        assert_eq!(
            extract_json_string(&value, "$.nested.url").as_deref(),
            Some("https://example.com/b.jpg")
        );
    }

    #[test]
    fn extract_json_string_supports_array_index_segments() {
        let value = serde_json::json!({
            "images": [{ "url": "https://example.com/first.jpg" }]
        });
        assert_eq!(
            extract_json_string(&value, "$.images[0].url").as_deref(),
            Some("https://example.com/first.jpg")
        );
    }

    #[test]
    fn extract_first_media_from_rss_reads_enclosure_url() {
        let xml = r#"<?xml version="1.0"?>
<rss><channel><item>
  <enclosure url="https://example.com/space.jpg" type="image/jpeg"/>
</item></channel></rss>"#;
        assert_eq!(
            extract_first_media_from_rss(xml).as_deref(),
            Some("https://example.com/space.jpg")
        );
    }

    #[test]
    fn extract_first_media_from_rss_reads_media_content_url() {
        let xml =
            r#"<item><media:content url='https://example.com/nebula.jpg' medium="image"/></item>"#;
        assert_eq!(
            extract_first_media_from_rss(xml).as_deref(),
            Some("https://example.com/nebula.jpg")
        );
    }
}

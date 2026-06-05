use serde::{Deserialize, Serialize};

use super::SourceEntry;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnsplashSourceConfig {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub orientation: Option<String>,
}

impl UnsplashSourceConfig {
    pub fn from_source(source: &SourceEntry) -> anyhow::Result<Self> {
        let mut config = Self {
            query: clean(source.query.clone()),
            collection: clean(source.collection.clone()),
            user: clean(source.user.clone()),
            topic: clean(source.topic.clone()),
            orientation: clean(source.orientation.clone()),
        };

        if let Some(url) = source.url.as_deref().filter(|url| !url.trim().is_empty()) {
            config.apply_url(url)?;
        }

        if config.collection.is_some() && config.query.is_some() {
            anyhow::bail!("Unsplash collection sources cannot also set query");
        }
        if config.topic.is_some() && config.query.is_some() {
            anyhow::bail!("Unsplash topic sources cannot also set query");
        }

        Ok(config)
    }

    fn apply_url(&mut self, url: &str) -> anyhow::Result<()> {
        let parsed = reqwest::Url::parse(url)?;
        let mut segments = parsed
            .path_segments()
            .map(Iterator::collect::<Vec<_>>)
            .unwrap_or_default();
        segments.retain(|segment| !segment.is_empty());

        match segments.as_slice() {
            ["collections", id] | ["collections", id, _] => {
                self.collection = Some((*id).to_string());
            }
            ["s", "photos", query] => {
                self.query = Some((*query).replace('-', " "));
            }
            ["t", topic] | ["t", topic, _] => {
                self.topic = Some((*topic).to_string());
            }
            [user] => {
                self.user = Some((*user).to_string());
            }
            _ => anyhow::bail!("unsupported Unsplash source URL: {url}"),
        }

        Ok(())
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

use std::path::PathBuf;

use anyhow::Context;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::Client;

use crate::config::reddit_json_url;
use crate::ctx::WallsCtx;
use crate::inline_providers::common::{
    self, download_bytes, first_enabled_source, provider_for, reddit_user_agent,
    write_cache_and_apply,
};
use crate::state::CurrentWallMetadata;

struct RedditCandidate {
    origin_url: String,
    image_url: String,
    author: Option<String>,
    title: Option<String>,
}

pub async fn try_reddit(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    let Some(src) = first_enabled_source(
        &ctx.config.sources,
        "reddit",
        true,
        ctx.config.change.internet_enabled,
    ) else {
        return Ok(None);
    };

    let provider = provider_for(src);
    let json_url =
        reddit_json_url(src).ok_or_else(|| anyhow::anyhow!("reddit source missing subreddit"))?;

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(reddit_user_agent()));

    let client = Client::builder().default_headers(headers).build()?;
    let listing: serde_json::Value = client
        .get(&json_url)
        .send()
        .await
        .with_context(|| provider.failure_scope("reddit listing fetch").to_string())?
        .json()
        .await
        .with_context(|| provider.failure_scope("reddit listing parse").to_string())?;

    let mut candidates = Vec::new();
    if let Some(children) = listing.pointer("/data/children").and_then(|v| v.as_array()) {
        for child in children {
            let Some(data) = child.get("data") else {
                continue;
            };
            if ctx.config.change.safe_mode
                && data.get("over_18").and_then(|v| v.as_bool()) == Some(true)
            {
                continue;
            }
            let Some(image_url) = reddit_post_image_url(data) else {
                continue;
            };
            let origin_url = data
                .get("permalink")
                .and_then(|v| v.as_str())
                .map(|p| format!("https://www.reddit.com{p}"))
                .unwrap_or_else(|| image_url.clone());
            candidates.push(RedditCandidate {
                origin_url,
                image_url,
                author: data
                    .get("author")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                title: data
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
    }

    let Some(candidate) = common::pick_random(&candidates) else {
        tracing::info!("reddit: no image posts found in listing");
        return Ok(None);
    };

    let bytes = download_bytes(
        &client,
        &candidate.image_url,
        &provider,
        "reddit image download",
    )
    .await?;
    let sub = crate::config::reddit_subreddit(src);
    let dest = write_cache_and_apply(
        ctx,
        "reddit-fetch.jpg",
        &bytes,
        format!("reddit:{sub}"),
        CurrentWallMetadata {
            provider: Some("reddit".into()),
            source_url: Some(candidate.origin_url.clone()),
            author: candidate.author.clone(),
            description: candidate.title.clone(),
        },
    )
    .await?;

    Ok(Some(dest))
}

fn reddit_post_image_url(data: &serde_json::Value) -> Option<String> {
    if let Some(url) = data.get("url").and_then(|v| v.as_str()) {
        let fixed = common::fix_imgur_url(url);
        if common::is_probably_image_url(&fixed) {
            return Some(fixed);
        }
    }
    data.pointer("/preview/images/0/source/url")
        .and_then(|v| v.as_str())
        .map(|url| url.replace("&amp;", "&"))
        .filter(|url| common::is_probably_image_url(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reddit_post_image_url_accepts_i_redd_it() {
        let data = serde_json::json!({
            "url": "https://i.redd.it/abc123.png"
        });
        assert_eq!(
            reddit_post_image_url(&data).as_deref(),
            Some("https://i.redd.it/abc123.png")
        );
    }

    #[test]
    fn reddit_post_image_url_fixes_imgur_page_links() {
        let data = serde_json::json!({
            "url": "https://imgur.com/abc123"
        });
        assert_eq!(
            reddit_post_image_url(&data).as_deref(),
            Some("https://i.imgur.com/abc123.jpg")
        );
    }
}

use std::path::PathBuf;

use anyhow::Context;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::Client;

use crate::config::{reddit_json_url, reddit_oauth_listing_url};
use crate::ctx::WallsCtx;
use crate::inline_providers::common::{
    self, api_base, download_bytes, provider_for, reddit_user_agent, write_cache_and_apply,
};
use crate::state::CurrentWallMetadata;

struct RedditCandidate {
    origin_url: String,
    image_url: String,
    author: Option<String>,
    title: Option<String>,
}

pub async fn try_reddit(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    if !ctx.config.change.internet_enabled {
        return Ok(None);
    }
    let Some(src) = ctx
        .config
        .sources
        .iter()
        .find(|s| s.enabled && s.source_type == "reddit")
        .cloned()
    else {
        return Ok(None);
    };

    match try_reddit_inner(ctx, &src).await {
        Ok(path) => Ok(path),
        Err(error) => {
            tracing::warn!(error = %error, "reddit: fetch failed, trying next source");
            Ok(None)
        }
    }
}

async fn try_reddit_inner(
    ctx: &mut WallsCtx,
    src: &crate::config::SourceEntry,
) -> anyhow::Result<Option<PathBuf>> {
    let provider = provider_for(src);
    let client_id = ctx.secrets.reddit_client_id.trim();
    let client_secret = ctx.secrets.reddit_client_secret.trim();

    let (listing_url, access_token) = if client_id.is_empty() {
        tracing::info!(
            "reddit: no API credentials configured (set reddit_client_id in secrets.json); trying public endpoint"
        );
        let url = reddit_json_url(src)
            .ok_or_else(|| anyhow::anyhow!("reddit source missing subreddit"))?;
        (url, None)
    } else {
        let url = reddit_oauth_listing_url(src)
            .ok_or_else(|| anyhow::anyhow!("reddit source missing subreddit"))?;
        let token = fetch_reddit_access_token(client_id, client_secret, &provider).await?;
        (url, Some(token))
    };

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(reddit_user_agent()));
    if let Some(token) = &access_token {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("bearer {token}"))
                .context("invalid reddit access token")?,
        );
    }

    let client = Client::builder().default_headers(headers).build()?;
    let listing: serde_json::Value = client
        .get(&listing_url)
        .send()
        .await
        .with_context(|| provider.failure_scope("reddit listing fetch").to_string())?
        .error_for_status()
        .with_context(|| provider.failure_scope("reddit listing status").to_string())?
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

async fn fetch_reddit_access_token(
    client_id: &str,
    client_secret: &str,
    provider: &crate::providers::ProviderDescriptor,
) -> anyhow::Result<String> {
    let base = api_base("REDDIT_API_BASE", "https://www.reddit.com");
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(reddit_user_agent()));

    let client = Client::builder().default_headers(headers).build()?;
    let secret = if client_secret.is_empty() {
        None
    } else {
        Some(client_secret)
    };
    let response: serde_json::Value = client
        .post(format!("{base}/api/v1/access_token"))
        .basic_auth(client_id, secret)
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .with_context(|| provider.failure_scope("reddit token fetch").to_string())?
        .error_for_status()
        .with_context(|| provider.failure_scope("reddit token status").to_string())?
        .json()
        .await
        .with_context(|| provider.failure_scope("reddit token parse").to_string())?;

    response
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("reddit token response missing access_token"))
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

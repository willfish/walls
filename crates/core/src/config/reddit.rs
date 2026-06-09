use super::{SourceEntry, SourceKind};

pub const REDDIT_SORT_CHOICES: &[&str] = &["hot", "new", "top", "rising", "controversial"];
pub const REDDIT_TIME_CHOICES: &[&str] = &["hour", "day", "week", "month", "year", "all"];

const DEFAULT_SORT: &str = "hot";
const DEFAULT_TIME: &str = "week";

pub fn reddit_sort_needs_time(sort: &str) -> bool {
    matches!(sort, "top" | "controversial")
}

pub fn reddit_sort_value(entry: &SourceEntry) -> &str {
    entry
        .sort
        .as_deref()
        .filter(|s| REDDIT_SORT_CHOICES.contains(s))
        .unwrap_or(DEFAULT_SORT)
}

pub fn reddit_time_value(entry: &SourceEntry) -> &str {
    entry
        .time
        .as_deref()
        .filter(|s| REDDIT_TIME_CHOICES.contains(s))
        .unwrap_or(DEFAULT_TIME)
}

/// Human-readable summary for lists: `r/wallpapers, top/month`.
pub fn reddit_summary(entry: &SourceEntry) -> String {
    let sub = reddit_subreddit(entry);
    if sub.is_empty() {
        return "(no subreddit)".into();
    }
    let sort = reddit_sort_value(entry);
    if reddit_sort_needs_time(sort) {
        format!("r/{sub}, {sort}/{}", reddit_time_value(entry))
    } else {
        format!("r/{sub}, {sort}")
    }
}

pub fn reddit_subreddit(entry: &SourceEntry) -> String {
    entry
        .query
        .as_deref()
        .map(subreddit_from_query)
        .unwrap_or_default()
}

/// OAuth API listing URL (`oauth.reddit.com`) when credentials are configured.
pub fn reddit_oauth_listing_url(entry: &SourceEntry) -> Option<String> {
    let sub = reddit_subreddit(entry);
    if sub.is_empty() {
        return None;
    }
    let sub = sub.trim().trim_start_matches("r/").trim_matches('/');
    let sort = reddit_sort_value(entry);
    let time = reddit_time_value(entry);
    let base = reddit_oauth_origin();
    let url = match sort {
        "new" => format!("{base}/r/{sub}/new.json?limit=100"),
        "rising" => format!("{base}/r/{sub}/rising.json?limit=100"),
        "top" => format!("{base}/r/{sub}/top.json?t={time}&limit=100"),
        "controversial" => format!("{base}/r/{sub}/controversial.json?t={time}&limit=100"),
        _ => format!("{base}/r/{sub}/hot.json?limit=100"),
    };
    Some(url)
}

/// Public JSON listing URL (Variety-compatible) for the Reddit downloader.
pub fn reddit_json_url(entry: &SourceEntry) -> Option<String> {
    let listing = reddit_listing_url(entry)?;
    if let Some((base, query)) = listing.split_once('?') {
        Some(format!(
            "{base}.json?{query}{}limit=100",
            if query.is_empty() { "" } else { "&" }
        ))
    } else {
        Some(format!("{listing}.json?limit=100"))
    }
}

/// Variety-compatible listing URL used by the Reddit downloader.
pub fn reddit_listing_url(entry: &SourceEntry) -> Option<String> {
    let sub = reddit_subreddit(entry);
    if sub.is_empty() {
        return None;
    }
    Some(build_listing_url(
        &sub,
        reddit_sort_value(entry),
        reddit_time_value(entry),
    ))
}

/// Split legacy `query` URLs into subreddit + sort + time fields for editing.
pub fn normalize_reddit_source(entry: &mut SourceEntry) {
    if SourceKind::parse(&entry.source_type) != SourceKind::Reddit {
        return;
    }

    let query = entry.query.clone().unwrap_or_default();
    if query.is_empty() {
        apply_reddit_defaults(entry);
        return;
    }

    if looks_like_reddit_url(&query) {
        let (sub, sort, time) = parse_reddit_url(&query);
        if !sub.is_empty() {
            entry.query = Some(sub);
        }
        if entry.sort.is_none() && !sort.is_empty() {
            entry.sort = Some(sort);
        }
        if entry.time.is_none() && !time.is_empty() {
            entry.time = Some(time);
        }
    } else {
        let sub = subreddit_from_query(&query);
        if !sub.is_empty() {
            entry.query = Some(sub);
        }
    }

    apply_reddit_defaults(entry);
    if !reddit_sort_needs_time(reddit_sort_value(entry)) {
        entry.time = None;
    }
}

fn apply_reddit_defaults(entry: &mut SourceEntry) {
    if entry
        .sort
        .as_deref()
        .is_none_or(|s| !REDDIT_SORT_CHOICES.contains(&s))
    {
        entry.sort = Some(DEFAULT_SORT.into());
    }
    if reddit_sort_needs_time(reddit_sort_value(entry))
        && entry
            .time
            .as_deref()
            .is_none_or(|s| !REDDIT_TIME_CHOICES.contains(&s))
    {
        entry.time = Some(DEFAULT_TIME.into());
    }
}

fn subreddit_from_query(query: &str) -> String {
    let trimmed = query.trim();
    if looks_like_reddit_url(trimmed) {
        return parse_reddit_url(trimmed).0;
    }
    trimmed
        .trim_start_matches("r/")
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

fn looks_like_reddit_url(query: &str) -> bool {
    let lower = query.to_ascii_lowercase();
    lower.contains("reddit.com")
}

fn parse_reddit_url(url: &str) -> (String, String, String) {
    let mut sort = DEFAULT_SORT.to_string();
    let mut time = String::new();

    let path = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url);
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let sub = segments
        .windows(2)
        .find_map(|w| (w[0] == "r").then_some(w[1].to_string()))
        .unwrap_or_default();

    for (idx, seg) in segments.iter().enumerate() {
        match *seg {
            "new" => sort = "new".into(),
            "rising" => sort = "rising".into(),
            "top" => sort = "top".into(),
            "controversial" => sort = "controversial".into(),
            "hot" if idx > 0 && segments.get(idx - 1) == Some(&"r") => sort = "hot".into(),
            _ => {}
        }
    }

    if let Some(query) = url.split('?').nth(1) {
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').map_or((pair, ""), |(k, v)| (k, v));
            if key == "t" && REDDIT_TIME_CHOICES.contains(&value) {
                time = value.to_string();
            }
            if key == "sort" && REDDIT_SORT_CHOICES.contains(&value) {
                sort = value.to_string();
            }
        }
    }

    if (sort == "top" || sort == "controversial") && time.is_empty() {
        time = DEFAULT_TIME.into();
    }

    (sub, sort, time)
}

fn reddit_origin() -> String {
    std::env::var("REDDIT_API_BASE")
        .unwrap_or_else(|_| "https://www.reddit.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn reddit_oauth_origin() -> String {
    std::env::var("REDDIT_OAUTH_API_BASE")
        .unwrap_or_else(|_| "https://oauth.reddit.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn build_listing_url(subreddit: &str, sort: &str, time: &str) -> String {
    let sub = subreddit.trim().trim_start_matches("r/").trim_matches('/');
    let base = format!("{}/r/{sub}/", reddit_origin());
    match sort {
        "new" => format!("{base}new/"),
        "rising" => format!("{base}rising/"),
        "top" => format!("{base}top/?sort=top&t={time}"),
        "controversial" => format!("{base}controversial/?sort=controversial&t={time}"),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reddit_entry(query: &str) -> SourceEntry {
        SourceEntry {
            enabled: true,
            source_type: "reddit".into(),
            label: None,
            path: None,
            query: Some(query.into()),
            url: None,
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
        }
    }

    #[test]
    fn json_url_appends_limit_query() {
        let entry = reddit_entry("wallpapers");
        assert_eq!(
            reddit_json_url(&entry).as_deref(),
            Some("https://www.reddit.com/r/wallpapers/.json?limit=100")
        );
    }

    #[test]
    fn oauth_listing_url_for_hot_subreddit() {
        let entry = reddit_entry("wallpapers");
        assert_eq!(
            reddit_oauth_listing_url(&entry).as_deref(),
            Some("https://oauth.reddit.com/r/wallpapers/hot.json?limit=100")
        );
    }

    #[test]
    fn listing_url_for_hot_subreddit() {
        let entry = reddit_entry("wallpapers");
        assert_eq!(
            reddit_listing_url(&entry).as_deref(),
            Some("https://www.reddit.com/r/wallpapers/")
        );
    }

    #[test]
    fn listing_url_for_top_with_time() {
        let mut entry = reddit_entry("comics");
        entry.sort = Some("top".into());
        entry.time = Some("month".into());
        assert_eq!(
            reddit_listing_url(&entry).as_deref(),
            Some("https://www.reddit.com/r/comics/top/?sort=top&t=month")
        );
    }

    #[test]
    fn normalize_splits_legacy_url_query() {
        let mut entry = reddit_entry("https://www.reddit.com/r/AutumnPorn/top/?sort=top&t=month");
        normalize_reddit_source(&mut entry);
        assert_eq!(entry.query.as_deref(), Some("AutumnPorn"));
        assert_eq!(entry.sort.as_deref(), Some("top"));
        assert_eq!(entry.time.as_deref(), Some("month"));
    }

    #[test]
    fn normalize_clears_time_for_hot() {
        let mut entry = reddit_entry("pics");
        entry.sort = Some("hot".into());
        entry.time = Some("week".into());
        normalize_reddit_source(&mut entry);
        assert_eq!(entry.time, None);
    }
}

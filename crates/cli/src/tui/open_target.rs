use std::path::{Path, PathBuf};
use std::process::Command;

use walls_core::config::{SourceEntry, SourceKind, WallhavenSearch};
use walls_core::{expand_home, WallsCtx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenTarget {
    Path(PathBuf),
    Url(String),
}

impl OpenTarget {
    pub(crate) fn display_value(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Url(url) => url.clone(),
        }
    }
}

pub(crate) fn spawn(target: &OpenTarget) -> anyhow::Result<()> {
    let command = open_command(target);
    Command::new(&command.program).args(&command.args).spawn()?;
    Ok(())
}

pub(crate) fn source(ctx: &WallsCtx, source: &SourceEntry) -> Option<OpenTarget> {
    if source.source_type == "wallhaven" {
        return Some(wallhaven_search(&ctx.config.wallhaven.search));
    }

    match SourceKind::parse(&source.source_type) {
        SourceKind::Folder | SourceKind::Image => source.path.as_deref().map(path_target),
        SourceKind::Favorites => Some(OpenTarget::Path(ctx.paths.favorites_dir.clone())),
        SourceKind::Fetched => Some(OpenTarget::Path(ctx.paths.fetched_dir.clone())),
        _ => source_url_target(source),
    }
}

pub(crate) fn cache_queue_id(cache_dir: &Path, id: &str) -> OpenTarget {
    if let Some(photo_id) = walls_core::unsplash::queue_photo_id(id) {
        if let Some(path) = walls_core::unsplash::cached_photo_path(cache_dir, photo_id) {
            return OpenTarget::Path(path);
        }
        return OpenTarget::Url(format!("https://unsplash.com/photos/{photo_id}"));
    }

    if let Some(path) = walls_core::wallhaven::cached_wallpaper_path(cache_dir, id) {
        return OpenTarget::Path(path);
    }
    wallhaven_wallpaper(id)
}

pub(crate) fn wallhaven_wallpaper(id: &str) -> OpenTarget {
    OpenTarget::Url(format!("https://wallhaven.cc/w/{id}"))
}

pub(crate) fn wallhaven_search(search: &WallhavenSearch) -> OpenTarget {
    OpenTarget::Url(format!(
        "https://wallhaven.cc/search?q={}&categories={}&purity={}&sorting={}&order={}&atleast={}",
        url_component(&search.q),
        search.categories,
        search.purity,
        search.sorting,
        search.order,
        url_component(&search.atleast)
    ))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OpenCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

pub(crate) fn open_command(target: &OpenTarget) -> OpenCommand {
    let value = target.display_value();
    #[cfg(target_os = "macos")]
    {
        OpenCommand {
            program: "open".into(),
            args: vec![value],
        }
    }

    #[cfg(target_os = "windows")]
    {
        OpenCommand {
            program: "cmd".into(),
            args: vec!["/C".into(), "start".into(), "".into(), value],
        }
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        OpenCommand {
            program: "xdg-open".into(),
            args: vec![value],
        }
    }
}

fn path_target(path: impl AsRef<Path>) -> OpenTarget {
    OpenTarget::Path(expand_home(path.as_ref()))
}

fn url_target(url: impl Into<String>) -> Option<OpenTarget> {
    let url = url.into();
    (!url.trim().is_empty()).then_some(OpenTarget::Url(url))
}

fn source_url_target(source: &SourceEntry) -> Option<OpenTarget> {
    if let Some(url) = source.url.as_deref().and_then(url_target) {
        return Some(url);
    }

    match SourceKind::parse(&source.source_type) {
        SourceKind::Reddit => walls_core::config::reddit_listing_url(source).map(OpenTarget::Url),
        SourceKind::Unsplash => source
            .collection
            .as_deref()
            .map(|collection| format!("https://unsplash.com/collections/{collection}"))
            .or_else(|| {
                source
                    .user
                    .as_deref()
                    .map(|user| format!("https://unsplash.com/@{user}"))
            })
            .or_else(|| {
                source
                    .topic
                    .as_deref()
                    .map(|topic| format!("https://unsplash.com/t/{topic}"))
            })
            .or_else(|| {
                source
                    .query
                    .as_deref()
                    .map(|query| format!("https://unsplash.com/s/photos/{}", url_component(query)))
            })
            .map(OpenTarget::Url),
        SourceKind::Pixabay => source.query.as_deref().map(|query| {
            OpenTarget::Url(format!(
                "https://pixabay.com/images/search/{}/",
                url_component(query)
            ))
        }),
        SourceKind::Apod => Some(OpenTarget::Url(
            "https://apod.nasa.gov/apod/astropix.html".into(),
        )),
        _ => None,
    }
}

fn url_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

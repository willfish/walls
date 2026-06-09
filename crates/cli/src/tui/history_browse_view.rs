use std::path::{Path, PathBuf};

use super::style::{self, StateKind};

pub(super) fn history_lines(history: &[String], cursor: usize) -> Vec<String> {
    let lines: Vec<String> = history
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mark = if i == cursor { ">" } else { " " };
            format!("{mark} {h}")
        })
        .collect();
    if lines.is_empty() {
        vec![style::state_text(
            StateKind::Empty,
            "no wallpaper history captured yet",
        )]
    } else {
        lines
    }
}

pub(super) fn selected_history_preview_path(history: &[String], cursor: usize) -> Option<PathBuf> {
    history
        .get(cursor)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

pub(super) fn browse_lines(items: Vec<String>, cursor: usize) -> Vec<String> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let mark = if i == cursor { ">" } else { " " };
            format!("{mark} {line}")
        })
        .collect()
}

pub(super) fn browse_items(
    queue: &[String],
    local_candidates: &[PathBuf],
    history: &[String],
) -> Vec<String> {
    let mut items = Vec::new();
    items.push("-- cache queue --".into());
    if queue.is_empty() {
        items.push(style::state_text(StateKind::Empty, "queue is empty"));
    } else {
        for id in queue {
            items.push(format!("queue: {id}"));
        }
    }
    items.push("-- local folders --".into());
    if local_candidates.is_empty() {
        items.push(style::state_text(
            StateKind::Empty,
            "no local candidates found",
        ));
    } else {
        for path in local_candidates {
            items.push(format!("local: {}", path.display()));
        }
    }
    items.push("-- history --".into());
    if history.is_empty() {
        items.push(style::state_text(
            StateKind::Empty,
            "no wallpaper history captured yet",
        ));
    } else {
        for h in history {
            items.push(format!("history: {h}"));
        }
    }
    items
}

pub(super) fn selected_browse_preview_path(
    items: Vec<String>,
    cursor: usize,
    cache_dir: &Path,
) -> Option<PathBuf> {
    let line = items.get(cursor)?;
    browse_preview_path_for_line(line, cache_dir)
}

fn browse_preview_path_for_line(line: &str, cache_dir: &Path) -> Option<PathBuf> {
    if let Some(path) = line
        .strip_prefix("local: ")
        .or_else(|| line.strip_prefix("history: "))
    {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }

    let id = line.strip_prefix("queue: ")?;
    if let Some(photo_id) = walls_core::unsplash::queue_photo_id(id) {
        return walls_core::unsplash::cached_photo_path(cache_dir, photo_id);
    }
    walls_core::wallhaven::cached_wallpaper_path(cache_dir, id)
}

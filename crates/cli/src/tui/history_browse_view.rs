use std::path::{Path, PathBuf};

use super::style::{self, StateKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowseRow {
    pub kind: BrowseRowKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowseRowKind {
    Section(&'static str),
    Empty(String),
    Queue(String),
    Local(PathBuf),
    History(PathBuf),
}

impl BrowseRow {
    pub(crate) fn label(&self) -> String {
        match &self.kind {
            BrowseRowKind::Section(label) => (*label).into(),
            BrowseRowKind::Empty(label) => label.clone(),
            BrowseRowKind::Queue(id) => format!("queue: {id}"),
            BrowseRowKind::Local(path) => format!("local: {}", path.display()),
            BrowseRowKind::History(path) => format!("history: {}", path.display()),
        }
    }

    pub(crate) fn preview_path(&self, cache_dir: &Path) -> Option<PathBuf> {
        match &self.kind {
            BrowseRowKind::Local(path) | BrowseRowKind::History(path) => {
                path.is_file().then_some(path.clone())
            }
            BrowseRowKind::Queue(id) => {
                if let Some(photo_id) = walls_core::unsplash::queue_photo_id(id) {
                    return walls_core::unsplash::cached_photo_path(cache_dir, photo_id);
                }
                walls_core::wallhaven::cached_wallpaper_path(cache_dir, id)
            }
            BrowseRowKind::Section(_) | BrowseRowKind::Empty(_) => None,
        }
    }
}

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

pub(super) fn browse_lines(rows: &[BrowseRow], cursor: usize) -> Vec<String> {
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let mark = if i == cursor { ">" } else { " " };
            format!("{mark} {}", row.label())
        })
        .collect()
}

pub(super) fn browse_rows(
    queue: &[String],
    local_candidates: &[PathBuf],
    history: &[String],
) -> Vec<BrowseRow> {
    let mut rows = Vec::new();
    rows.push(BrowseRow {
        kind: BrowseRowKind::Section("-- cache queue --"),
    });
    if queue.is_empty() {
        rows.push(BrowseRow {
            kind: BrowseRowKind::Empty(style::state_text(StateKind::Empty, "queue is empty")),
        });
    } else {
        for id in queue {
            rows.push(BrowseRow {
                kind: BrowseRowKind::Queue(id.clone()),
            });
        }
    }
    rows.push(BrowseRow {
        kind: BrowseRowKind::Section("-- local folders --"),
    });
    if local_candidates.is_empty() {
        rows.push(BrowseRow {
            kind: BrowseRowKind::Empty(style::state_text(
                StateKind::Empty,
                "no local candidates found",
            )),
        });
    } else {
        for path in local_candidates {
            rows.push(BrowseRow {
                kind: BrowseRowKind::Local(path.clone()),
            });
        }
    }
    rows.push(BrowseRow {
        kind: BrowseRowKind::Section("-- history --"),
    });
    if history.is_empty() {
        rows.push(BrowseRow {
            kind: BrowseRowKind::Empty(style::state_text(
                StateKind::Empty,
                "no wallpaper history captured yet",
            )),
        });
    } else {
        for h in history {
            rows.push(BrowseRow {
                kind: BrowseRowKind::History(PathBuf::from(h)),
            });
        }
    }
    rows
}

pub(super) fn selected_browse_preview_path(
    rows: &[BrowseRow],
    cursor: usize,
    cache_dir: &Path,
) -> Option<PathBuf> {
    rows.get(cursor)?.preview_path(cache_dir)
}

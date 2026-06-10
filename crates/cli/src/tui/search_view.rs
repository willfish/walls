use walls_core::config::WallhavenSearch;

use super::app::SearchHit;
use super::style::{self, StateKind};

pub(super) fn lines(
    query: &str,
    filters: &WallhavenSearch,
    results: &[SearchHit],
    cursor: usize,
) -> Vec<String> {
    let mut lines = vec![
        format!("provider: Wallhaven | query: {query}"),
        format!(
            "filters: purity {} | categories {} | ratio {} | sorting {} {} | minimum {}",
            filters.purity,
            filters.categories,
            filters.ratios,
            filters.sorting,
            filters.order,
            filters.atleast
        ),
        "edit: / or i query | e filters".into(),
    ];
    if results.is_empty() {
        lines.push(style::state_text(
            StateKind::Empty,
            "no results; press / or i to edit query, Enter to search",
        ));
    } else {
        for (i, hit) in results.iter().enumerate() {
            let mark = if i == cursor { ">" } else { " " };
            lines.push(format!("{mark} Wallhaven {} — {}", hit.id, hit.label));
        }
    }
    lines
}

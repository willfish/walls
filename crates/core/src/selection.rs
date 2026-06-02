use std::collections::HashSet;

use rand::seq::SliceRandom;

pub struct PickInput<'a> {
    pub candidates: &'a [String],
    pub recent: &'a [String],
    pub avoid_recent: usize,
}

pub fn pick_next(input: &PickInput) -> anyhow::Result<String> {
    let recent_set: HashSet<_> = input.recent.iter().take(input.avoid_recent).collect();
    let pool: Vec<_> = input
        .candidates
        .iter()
        .filter(|c| !recent_set.contains(*c))
        .cloned()
        .collect();
    let pool = if pool.is_empty() {
        input.candidates.to_vec()
    } else {
        pool
    };
    pool.choose(&mut rand::thread_rng())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no candidates"))
}

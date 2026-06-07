//! In-process automatic wallpaper rotation (Variety-style interval from config).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::ChangeConfig;
use crate::ctx::WallsCtx;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickOutcome {
    Idle,
    Skipped,
    NoChange,
    Rotated,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct AutoRotator {
    on_start_done: bool,
}

impl AutoRotator {
    pub fn new() -> Self {
        Self {
            on_start_done: false,
        }
    }

    /// Evaluate config/state and run `advance_next` when due.
    pub async fn tick(&mut self, ctx: &mut WallsCtx) -> TickOutcome {
        let change = &ctx.config.change;
        let state = &ctx.state;

        // Allow `on_start` to fire again if the user toggles it off then on in config.
        if !change.on_start {
            self.on_start_done = false;
        }

        if should_run_on_start(self.on_start_done, change, state) {
            self.on_start_done = true;
            return Self::advance(ctx).await;
        }

        if rotation_due(state, change, unix_now()) {
            return Self::advance(ctx).await;
        }

        if !change.enabled || state.paused {
            TickOutcome::Skipped
        } else {
            TickOutcome::Idle
        }
    }

    async fn advance(ctx: &mut WallsCtx) -> TickOutcome {
        match ctx.advance_next().await {
            Ok(Some(_)) => TickOutcome::Rotated,
            Ok(None) => TickOutcome::NoChange,
            Err(err) => {
                tracing::warn!("auto-rotate failed: {err:#}");
                TickOutcome::Error
            }
        }
    }
}

impl Default for AutoRotator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn should_run_on_start(on_start_done: bool, change: &ChangeConfig, state: &State) -> bool {
    !on_start_done && change.enabled && change.on_start && !state.paused
}

pub fn rotation_due(state: &State, change: &ChangeConfig, now: u64) -> bool {
    if !change.enabled || state.paused || change.interval_secs == 0 {
        return false;
    }
    state.last_change_unix.saturating_add(change.interval_secs) <= now
}

/// Manual advance for tray/TUI/CLI explicit actions.
pub async fn advance_manual(ctx: &mut WallsCtx) -> anyhow::Result<Option<PathBuf>> {
    ctx.advance_next_manual().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ChangeConfig;

    fn change(enabled: bool, on_start: bool, interval_secs: u64) -> ChangeConfig {
        ChangeConfig {
            enabled,
            on_start,
            interval_secs,
            ..ChangeConfig::default()
        }
    }

    fn state(paused: bool, last_change_unix: u64) -> State {
        State {
            paused,
            last_change_unix,
            ..State::default()
        }
    }

    #[test]
    fn rotation_due_when_interval_elapsed() {
        let cfg = change(true, false, 300);
        let st = state(false, 1_000);
        assert!(rotation_due(&st, &cfg, 1_300));
        assert!(!rotation_due(&st, &cfg, 1_299));
    }

    #[test]
    fn rotation_not_due_when_paused_or_disabled_or_zero_interval() {
        let cfg = change(true, false, 60);
        let st = state(false, 0);
        assert!(!rotation_due(&state(true, 0), &cfg, 10_000));
        assert!(!rotation_due(&st, &change(false, false, 60), 10_000));
        assert!(!rotation_due(&st, &change(true, false, 0), 10_000));
    }

    #[test]
    fn on_start_runs_once_when_enabled_and_not_paused() {
        let cfg = change(true, true, 300);
        assert!(should_run_on_start(false, &cfg, &state(false, 0)));
        assert!(!should_run_on_start(true, &cfg, &state(false, 0)));
        assert!(!should_run_on_start(false, &cfg, &state(true, 0)));
        assert!(!should_run_on_start(
            false,
            &change(true, false, 300),
            &state(false, 0)
        ));
    }
}

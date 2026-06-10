//! Poll-driven automatic rotation using `change.interval_secs` from config.

use std::sync::OnceLock;

use tokio::runtime::Runtime;
use walls_core::config::ChangeConfig;
use walls_core::rotation::{AutoRotator, TickOutcome};
use walls_core::WallsCtx;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("walls-tray tokio runtime")
    })
}

pub struct RotationLoop {
    rotator: AutoRotator,
    last_interval_secs: Option<u64>,
    last_enabled: Option<bool>,
}

impl RotationLoop {
    pub fn new() -> Self {
        Self {
            rotator: AutoRotator::new(),
            last_interval_secs: None,
            last_enabled: None,
        }
    }

    /// Called from the tray main loop (~200ms). Reloads `config.json` and state each tick.
    /// Missing `config.json` is recreated with defaults (see `load_or_create_config` in core).
    pub fn poll(&mut self) {
        let outcome = runtime().block_on(async {
            let mut ctx = WallsCtx::load()?;
            self.log_config_changes(&ctx.config.change);
            Ok::<_, anyhow::Error>(self.rotator.tick(&mut ctx).await)
        });

        match outcome {
            Ok(outcome) => {
                if let Some(message) = tick_outcome_message(outcome) {
                    tracing::info!("{message}");
                }
            }
            Err(err) => tracing::warn!("auto-rotate tick failed: {err:#}"),
        }
    }

    fn log_config_changes(&mut self, change: &ChangeConfig) {
        if self.record_config_snapshot(change) {
            tracing::info!(
                "rotation config: enabled={} interval_secs={} on_start={}",
                change.enabled,
                change.interval_secs,
                change.on_start
            );
        }
    }

    fn record_config_snapshot(&mut self, change: &ChangeConfig) -> bool {
        let changed = self.last_interval_secs != Some(change.interval_secs)
            || self.last_enabled != Some(change.enabled);
        if changed {
            self.last_interval_secs = Some(change.interval_secs);
            self.last_enabled = Some(change.enabled);
        }
        changed
    }
}

impl Default for RotationLoop {
    fn default() -> Self {
        Self::new()
    }
}

/// Tray menu "Next" — explicit user action, ignores pause/rotation-off.
pub fn advance_manual() -> anyhow::Result<Option<std::path::PathBuf>> {
    let result = runtime().block_on(async {
        let mut ctx = WallsCtx::load()?;
        walls_core::rotation::advance_manual(&mut ctx).await
    });
    match result {
        Ok(Some(path)) => {
            tracing::info!("manual next: {}", path.display());
            Ok(Some(path))
        }
        Ok(None) => {
            tracing::info!("manual next: no change");
            Ok(None)
        }
        Err(err) => {
            tracing::warn!("manual next failed: {err:#}");
            Err(err)
        }
    }
}

fn tick_outcome_message(outcome: TickOutcome) -> Option<&'static str> {
    match outcome {
        TickOutcome::Rotated => Some("auto-rotated wallpaper"),
        TickOutcome::NoChange => Some("auto-rotate: due but no wallpaper change"),
        TickOutcome::Idle | TickOutcome::Skipped | TickOutcome::Error => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rotation_loop_has_no_config_snapshot() {
        let loop_state = RotationLoop::default();

        assert_eq!(loop_state.last_interval_secs, None);
        assert_eq!(loop_state.last_enabled, None);
    }

    #[test]
    fn config_snapshot_changes_for_initial_interval_or_enabled_updates() {
        let mut loop_state = RotationLoop::new();
        let mut change = ChangeConfig::default();

        assert!(loop_state.record_config_snapshot(&change));
        assert!(!loop_state.record_config_snapshot(&change));

        change.on_start = !change.on_start;
        assert!(!loop_state.record_config_snapshot(&change));

        change.interval_secs += 1;
        assert!(loop_state.record_config_snapshot(&change));
        assert!(!loop_state.record_config_snapshot(&change));

        change.enabled = !change.enabled;
        assert!(loop_state.record_config_snapshot(&change));
        assert!(!loop_state.record_config_snapshot(&change));
    }

    #[test]
    fn tick_outcome_messages_cover_loggable_and_silent_states() {
        assert_eq!(
            tick_outcome_message(TickOutcome::Rotated),
            Some("auto-rotated wallpaper")
        );
        assert_eq!(
            tick_outcome_message(TickOutcome::NoChange),
            Some("auto-rotate: due but no wallpaper change")
        );
        assert_eq!(tick_outcome_message(TickOutcome::Idle), None);
        assert_eq!(tick_outcome_message(TickOutcome::Skipped), None);
        assert_eq!(tick_outcome_message(TickOutcome::Error), None);
    }
}

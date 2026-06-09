//! Poll-driven automatic rotation using `change.interval_secs` from config.

use std::sync::OnceLock;

use tokio::runtime::Runtime;
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
            Ok(TickOutcome::Rotated) => tracing::info!("auto-rotated wallpaper"),
            Ok(TickOutcome::NoChange) => tracing::info!("auto-rotate: due but no wallpaper change"),
            Ok(TickOutcome::Idle | TickOutcome::Skipped) => {}
            Ok(TickOutcome::Error) => {}
            Err(err) => tracing::warn!("auto-rotate tick failed: {err:#}"),
        }
    }

    fn log_config_changes(&mut self, change: &walls_core::config::ChangeConfig) {
        let interval = change.interval_secs;
        let enabled = change.enabled;
        if self.last_interval_secs != Some(interval) || self.last_enabled != Some(enabled) {
            tracing::info!(
                "rotation config: enabled={enabled} interval_secs={interval} on_start={}",
                change.on_start
            );
            self.last_interval_secs = Some(interval);
            self.last_enabled = Some(enabled);
        }
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

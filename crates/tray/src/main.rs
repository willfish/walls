use tracing_subscriber::EnvFilter;
use walls_tray::{lock, platform, resolve_walls_bin};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walls_tray=info".parse()?))
        .init();

    let lock_path = match walls_core::paths::WallsPaths::discover() {
        Ok(paths) => paths.config_dir.join("tray.lock"),
        Err(_) => std::env::temp_dir().join("walls-tray.lock"),
    };
    let _tray_lock = lock::acquire_tray_lock(&lock_path)?;

    tracing::info!(
        "walls-tray using walls binary at {}",
        resolve_walls_bin().display()
    );

    if platform::prefer_status_notifier() {
        match walls_tray::sni::run() {
            Ok(()) => return Ok(()),
            Err(err) => {
                tracing::warn!("StatusNotifier tray unavailable ({err}); trying AppIndicator");
            }
        }
    }

    if platform::is_wayland_session() {
        tracing::error!(
            "no StatusNotifier tray host on this Wayland session; use the TUI (runs its own scheduler when tray is unavailable)"
        );
        return Ok(());
    }

    walls_tray::appindicator::run()
}

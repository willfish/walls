use super::WallsCtx;
use crate::config::{load_or_create_config, load_secrets};
use crate::error::{Result, WallsError};
use crate::paths::WallsPaths;
use crate::providers::ProviderStatusReport;
use crate::state::State;
use std::path::Path;

impl WallsCtx {
    pub fn load() -> Result<Self> {
        let paths =
            WallsPaths::discover().map_err(|source| WallsError::PathDiscovery { source })?;
        Self::load_with_paths(paths)
    }

    /// Load config/state from a test or alternate root directory.
    pub fn load_from(root: &Path) -> Result<Self> {
        let paths = WallsPaths {
            config_dir: root.to_path_buf(),
            config_file: root.join("config.json"),
            secrets_file: root.join("secrets.json"),
            state_file: root.join("state.json"),
            cache_dir: root.join("cache"),
            download_dir: root.join("downloaded"),
            favorites_dir: root.join("favorites"),
            fetched_dir: root.join("fetched"),
            compose_dir: root.join("wallpaper"),
        };
        Self::load_with_paths(paths)
    }

    pub fn load_with_paths(mut paths: WallsPaths) -> Result<Self> {
        let config =
            load_or_create_config(&paths.config_file).map_err(|source| WallsError::ConfigLoad {
                path: paths.config_file.clone(),
                source,
            })?;
        let secrets =
            load_secrets(&paths.secrets_file).map_err(|source| WallsError::SecretsLoad {
                path: paths.secrets_file.clone(),
                source,
            })?;
        paths.apply_config_paths(&config.paths);
        paths
            .ensure_data_dirs()
            .map_err(|source| WallsError::DataDirCreate { source })?;
        let state =
            State::load_or_default(&paths.state_file).map_err(|source| WallsError::StateLoad {
                path: paths.state_file.clone(),
                source,
            })?;
        let ctx = Self {
            paths,
            config,
            secrets,
            state,
            provider_status_report: ProviderStatusReport::default(),
        };
        crate::validate::warn_validation_issues(&ctx.config, &ctx.secrets, &ctx.paths);
        if let Err(err) = crate::autostart::sync_tray_autostart(&ctx.config) {
            tracing::warn!("tray autostart sync failed: {err:#}");
        }
        Ok(ctx)
    }

    pub fn save_state(&self) -> anyhow::Result<()> {
        self.state.save(&self.paths.state_file)
    }

    pub(super) fn with_state_lock<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file)?;
        self.state = State::load_or_default(&self.paths.state_file)?;
        f(self)
    }

    pub(super) fn with_typed_state_lock<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<R>,
    ) -> Result<R> {
        let _lock = crate::lock::StateLock::acquire(&self.paths.state_file).map_err(|source| {
            WallsError::StateLock {
                path: self.paths.state_file.clone(),
                source,
            }
        })?;
        self.state = State::load_or_default(&self.paths.state_file).map_err(|source| {
            WallsError::StateLoad {
                path: self.paths.state_file.clone(),
                source,
            }
        })?;
        f(self)
    }
}

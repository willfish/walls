use std::path::PathBuf;

/// Result type returned by typed `walls-core` public APIs.
pub type Result<T> = std::result::Result<T, WallsError>;

/// Typed errors returned by the public `walls-core` context API.
#[derive(Debug, thiserror::Error)]
pub enum WallsError {
    /// The platform config or state location could not be discovered.
    #[error("failed to discover walls paths: {source}")]
    PathDiscovery {
        #[source]
        source: anyhow::Error,
    },

    /// The user config file could not be read or parsed.
    #[error("failed to load {}: {source}", path.display())]
    ConfigLoad {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// The optional secrets file existed but could not be read or parsed.
    #[error("failed to load {}: {source}", path.display())]
    SecretsLoad {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// Configured data directories or the state directory could not be created.
    #[error("failed to prepare walls data directories: {source}")]
    DataDirCreate {
        #[source]
        source: anyhow::Error,
    },

    /// The state file could not be read or parsed while loading context.
    #[error("failed to load {}: {source}", path.display())]
    StateLoad {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// The state file lock could not be acquired before a state-changing operation.
    #[error("failed to lock {}: {source}", path.display())]
    StateLock {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// Applying a wallpaper failed during compose, backend apply, or state save.
    #[error("failed to apply wallpaper {}: {source}", original.display())]
    ApplyFile {
        original: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// Refreshing the current wallpaper failed during compose, backend apply, or state save.
    #[error("failed to refresh current wallpaper: {source}")]
    RefreshCurrent {
        #[source]
        source: anyhow::Error,
    },

    /// The state points at an original wallpaper file that no longer exists.
    #[error("current original wallpaper does not exist: {}", path.display())]
    CurrentOriginalMissing { path: PathBuf },

    /// The state points at a composed wallpaper file that no longer exists.
    #[error("current composed wallpaper does not exist: {}", path.display())]
    CurrentComposedMissing { path: PathBuf },
}

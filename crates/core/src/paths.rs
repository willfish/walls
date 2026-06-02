use std::path::{Path, PathBuf};

/// XDG and expanded paths for walls.
#[derive(Debug, Clone)]
pub struct WallsPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub secrets_file: PathBuf,
    pub state_file: PathBuf,
    pub cache_dir: PathBuf,
    pub download_dir: PathBuf,
    pub favorites_dir: PathBuf,
    pub fetched_dir: PathBuf,
    pub compose_dir: PathBuf,
}

fn config_dir() -> anyhow::Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("walls"));
    }
    let proj = directories::ProjectDirs::from("", "", "walls")
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;
    Ok(proj.config_dir().to_path_buf())
}

fn state_file() -> anyhow::Result<PathBuf> {
    let base = if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(xdg)
    } else {
        dirs::state_dir().ok_or_else(|| anyhow::anyhow!("could not determine state directory"))?
    };
    Ok(base.join("walls").join("state.json"))
}

impl WallsPaths {
    pub fn discover() -> anyhow::Result<Self> {
        let config_dir = config_dir()?;
        let state_file = state_file()?;
        Ok(Self {
            config_file: config_dir.join("config.json"),
            secrets_file: config_dir.join("secrets.json"),
            config_dir,
            state_file,
            cache_dir: PathBuf::new(),
            download_dir: PathBuf::new(),
            favorites_dir: PathBuf::new(),
            fetched_dir: PathBuf::new(),
            compose_dir: PathBuf::new(),
        })
    }

    pub fn apply_config_paths(&mut self, paths: &crate::config::PathsConfig) {
        self.cache_dir = expand_home(&paths.cache_dir);
        self.download_dir = expand_home(&paths.download_dir);
        self.favorites_dir = expand_home(&paths.favorites_dir);
        self.fetched_dir = expand_home(&paths.fetched_dir);
        self.compose_dir = expand_home(&paths.compose_dir);
    }

    pub fn ensure_data_dirs(&self) -> anyhow::Result<()> {
        for dir in [
            &self.cache_dir,
            &self.download_dir,
            &self.favorites_dir,
            &self.fetched_dir,
            &self.compose_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        if let Some(parent) = self.state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

/// Expand a leading `~/` to the user home directory.
pub fn expand_home(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

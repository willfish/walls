use std::path::{Path, PathBuf};

/// XDG and expanded paths for walls.
#[derive(Debug, Clone)]
pub struct WallsPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub secrets_file: PathBuf,
    pub state_file: PathBuf,
    pub event_journal_file: PathBuf,
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

fn data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("walls");
    }
    if let Some(data) = dirs::data_local_dir() {
        return data.join("walls");
    }
    if let Some(home) = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
    {
        return home.join(".local").join("share").join("walls");
    }
    PathBuf::from(".")
        .join(".local")
        .join("share")
        .join("walls")
}

impl WallsPaths {
    pub fn discover() -> anyhow::Result<Self> {
        let config_dir = config_dir()?;
        let state_file = state_file()?;
        let event_journal_file = state_file.with_file_name("events.jsonl");
        Ok(Self {
            config_file: config_dir.join("config.json"),
            secrets_file: config_dir.join("secrets.json"),
            config_dir,
            state_file,
            event_journal_file,
            cache_dir: PathBuf::new(),
            download_dir: PathBuf::new(),
            favorites_dir: PathBuf::new(),
            fetched_dir: PathBuf::new(),
            compose_dir: PathBuf::new(),
        })
    }

    pub fn apply_config_paths(&mut self, paths: &crate::config::PathsConfig) {
        let data_dir = data_dir();
        self.cache_dir = resolve_data_path(&paths.cache_dir, &data_dir);
        self.download_dir = resolve_data_path(&paths.download_dir, &data_dir);
        self.favorites_dir = resolve_data_path(&paths.favorites_dir, &data_dir);
        self.fetched_dir = resolve_data_path(&paths.fetched_dir, &data_dir);
        self.compose_dir = resolve_data_path(&paths.compose_dir, &data_dir);
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

fn resolve_data_path(path: impl AsRef<Path>, data_dir: &Path) -> PathBuf {
    let path = expand_home(path);
    if path.is_absolute() {
        path
    } else {
        data_dir.join(path)
    }
}

/// Expand a leading `~/` to the user home directory.
pub fn expand_home(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = s.strip_prefix("~/") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir);
        if let Some(home) = home {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::resolve_data_path;
    use std::path::Path;

    #[test]
    fn relative_storage_paths_resolve_under_data_dir() {
        assert_eq!(
            resolve_data_path("cache", Path::new("/var/lib/walls")),
            Path::new("/var/lib/walls/cache")
        );
    }

    #[test]
    fn absolute_storage_paths_are_left_alone() {
        assert_eq!(
            resolve_data_path("/tmp/walls-cache", Path::new("/var/lib/walls")),
            Path::new("/tmp/walls-cache")
        );
    }
}

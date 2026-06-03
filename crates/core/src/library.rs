use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// Move `src` into `dest_dir`, picking a non-colliding filename.
pub fn move_into_dir(src: &Path, dest_dir: &Path) -> anyhow::Result<PathBuf> {
    if !src.is_file() {
        anyhow::bail!("not a file: {}", src.display());
    }
    fs::create_dir_all(dest_dir)?;
    let name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("path has no file name: {}", src.display()))?;
    let dest = unique_path(dest_dir, name);
    fs::rename(src, &dest)?;
    Ok(dest)
}

/// Copy `src` into `dest_dir`, picking a non-colliding filename.
pub fn copy_into_dir(src: &Path, dest_dir: &Path) -> anyhow::Result<PathBuf> {
    if !src.is_file() {
        anyhow::bail!("not a file: {}", src.display());
    }
    fs::create_dir_all(dest_dir)?;
    let name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("path has no file name: {}", src.display()))?;
    let dest = unique_path(dest_dir, name);
    fs::copy(src, &dest)?;
    Ok(dest)
}

fn unique_path(dir: &Path, name: &OsStr) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let stem = Path::new(name).file_stem().map(|s| s.to_os_string());
    let ext = Path::new(name).extension().map(|s| s.to_os_string());
    for n in 1..10_000 {
        let candidate = match (&stem, &ext) {
            (Some(stem), Some(ext)) => dir.join(format!(
                "{}_{}.{}",
                stem.to_string_lossy(),
                n,
                ext.to_string_lossy()
            )),
            (Some(stem), None) => dir.join(format!("{}_{}", stem.to_string_lossy(), n)),
            _ => dir.join(format!("file_{n}")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    dest
}

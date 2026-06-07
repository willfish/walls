use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use fs2::FileExt;

/// Guard for the tray singleton lock. Held for the lifetime of the tray process.
pub struct TrayLock {
    _file: File,
    pid_path: PathBuf,
}

/// Path alongside `tray.lock` where the running tray PID is recorded.
pub fn tray_pid_path(lock_path: &Path) -> PathBuf {
    lock_path.with_file_name("tray.pid")
}

/// Try to acquire an exclusive lock for the tray process.
/// Returns Err if another instance holds the lock.
pub fn try_acquire_tray_lock(path: &Path) -> anyhow::Result<TrayLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_exclusive()?;
    let pid_path = tray_pid_path(path);
    fs::write(&pid_path, std::process::id().to_string())?;
    Ok(TrayLock {
        _file: file,
        pid_path,
    })
}

/// Acquire the tray singleton lock, stopping any other running `walls-tray` first.
pub fn acquire_tray_lock(path: &Path) -> anyhow::Result<TrayLock> {
    match try_acquire_tray_lock(path) {
        Ok(guard) => Ok(guard),
        Err(first) => {
            tracing::info!("walls-tray already running; stopping previous instance");
            stop_running_tray(path)?;
            const MAX_ATTEMPTS: usize = 30;
            for attempt in 0..MAX_ATTEMPTS {
                thread::sleep(Duration::from_millis(100));
                if let Ok(guard) = try_acquire_tray_lock(path) {
                    return Ok(guard);
                }
                if attempt + 1 == MAX_ATTEMPTS {
                    return Err(first);
                }
            }
            Err(first)
        }
    }
}

/// Stop the tray process recorded in `tray.pid`, if it is still running.
pub fn stop_running_tray(lock_path: &Path) -> anyhow::Result<()> {
    let pid_path = tray_pid_path(lock_path);
    let Ok(pid_str) = fs::read_to_string(&pid_path) else {
        return Ok(());
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        let _ = fs::remove_file(&pid_path);
        return Ok(());
    };

    let my_pid = std::process::id();
    if pid != my_pid && proc_exists(pid) && is_walls_tray_process(pid) {
        signal_tray(pid)?;
        wait_for_tray_exit(&pid_path, pid);
        if proc_exists(pid) {
            signal_tray_kill(pid)?;
            wait_for_tray_exit(&pid_path, pid);
        }
    }

    let _ = fs::remove_file(&pid_path);
    Ok(())
}

fn is_walls_tray_process(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(exe) = fs::read_link(format!("/proc/{pid}/exe")) {
            return exe
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "walls-tray");
        }
        return false;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        true
    }
}

fn proc_exists(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn wait_for_tray_exit(pid_path: &Path, pid: u32) {
    for _ in 0..30 {
        if !proc_exists(pid) {
            break;
        }
        let pid_stale = fs::read_to_string(pid_path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .is_some_and(|recorded| recorded != pid);
        if pid_stale {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(unix)]
fn signal_tray(pid: u32) -> anyhow::Result<()> {
    use std::process::Command;
    let status = Command::new("kill").arg(pid.to_string()).status()?;
    if !status.success() {
        anyhow::bail!("kill {pid} failed with status {status}");
    }
    Ok(())
}

#[cfg(unix)]
fn signal_tray_kill(pid: u32) -> anyhow::Result<()> {
    use std::process::Command;
    let status = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status()?;
    if !status.success() {
        anyhow::bail!("kill -9 {pid} failed with status {status}");
    }
    Ok(())
}

#[cfg(not(unix))]
fn signal_tray(_pid: u32) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn signal_tray_kill(_pid: u32) -> anyhow::Result<()> {
    Ok(())
}

impl Drop for TrayLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.pid_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn try_acquire_fails_when_lock_held_by_another_handle() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("tray.lock");

        let _held = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        _held.try_lock_exclusive().unwrap();

        let result = try_acquire_tray_lock(&lock_path);
        assert!(
            result.is_err(),
            "expected lock acquisition to fail when held by another instance"
        );
    }

    #[test]
    fn stop_running_tray_cleans_stale_pid_when_process_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("tray.lock");
        let pid_path = tray_pid_path(&lock_path);
        fs::write(&pid_path, "999999999").unwrap();

        stop_running_tray(&lock_path).expect("stale pid cleanup");

        assert!(!pid_path.exists());
    }

    #[test]
    fn tray_lock_writes_and_removes_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("tray.lock");
        let pid_path = tray_pid_path(&lock_path);

        {
            let _guard = try_acquire_tray_lock(&lock_path).unwrap();
            let pid = fs::read_to_string(&pid_path).unwrap();
            assert_eq!(pid.trim(), std::process::id().to_string());
        }

        assert!(!pid_path.exists());
    }
}

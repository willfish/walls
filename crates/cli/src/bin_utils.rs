use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use walls_core::bin_resolve::{resolve_binary, BinResolveOpts};
use walls_core::paths::WallsPaths;
use walls_core::tray::{decide_tray_action, TrayAction};

/// Resolve the `walls-tray` binary for auto-starting the tray from the TUI/CLI.
pub fn resolve_tray_bin() -> PathBuf {
    resolve_tray_bin_from(
        std::env::var("WALLS_TRAY_BIN").ok().as_deref(),
        std::env::current_exe().ok().as_deref(),
    )
}

/// Testable resolver for `walls-tray` next to the walls binary, build tree, or `WALLS_TRAY_BIN`.
pub fn resolve_tray_bin_from(tray_bin_env: Option<&str>, current_exe: Option<&Path>) -> PathBuf {
    resolve_binary(BinResolveOpts {
        env_var: tray_bin_env,
        current_exe,
        sibling_name: "walls-tray",
        build_default: option_env!("WALLS_CLI_TRAY_DEFAULT"),
        path_fallback: "walls-tray",
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayRuntimeStatus {
    pub resolved_bin: PathBuf,
    pub resolved_bin_exists: bool,
    pub running: bool,
}

pub fn tray_runtime_status() -> TrayRuntimeStatus {
    let resolved_bin = resolve_tray_bin();
    TrayRuntimeStatus {
        resolved_bin_exists: resolved_bin.is_file(),
        resolved_bin,
        running: tray_is_running(),
    }
}

/// Outcome of attempting to ensure the tray is running from the TUI launch path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureTrayOutcome {
    Spawned,
    /// Tray already running; TUI must not spawn another.
    AlreadyRunning,
    Skipped {
        reason: String,
    },
    Failed {
        reason: String,
    },
}

impl EnsureTrayOutcome {
    /// User-facing TUI status line, if any.
    pub fn tui_message(&self) -> Option<String> {
        match self {
            Self::Spawned | Self::AlreadyRunning => None,
            Self::Skipped { reason } => Some(format!("tray: {reason}")),
            Self::Failed { reason } => Some(format!("tray: {reason}")),
        }
    }

    /// When the tray is running it owns the in-process rotation scheduler.
    pub fn owns_auto_rotation(&self) -> bool {
        matches!(self, Self::Spawned | Self::AlreadyRunning)
    }
}

/// Ensure the tray is running when the desktop supports it (fire-and-forget spawn).
/// Does not replace an already-running tray; start `walls-tray` directly to do that.
pub fn ensure_tray_running() -> EnsureTrayOutcome {
    match decide_tray_action() {
        TrayAction::Skip { reason } => {
            tracing::info!("skipping walls-tray auto-start: {reason}");
            EnsureTrayOutcome::Skipped { reason }
        }
        TrayAction::Spawn => {
            if tray_is_running() {
                tracing::info!("walls-tray already running; not starting another from TUI");
                return EnsureTrayOutcome::AlreadyRunning;
            }
            let tray = resolve_tray_bin();
            build_tray_for_dev_if_needed(&tray);
            spawn_tray()
        }
    }
}

/// Build `walls-tray` from the repo when `cargo run` is using `target/*/walls`.
fn build_tray_for_dev_if_needed(tray: &Path) {
    if !tray_is_dev_target(tray) {
        return;
    }
    let workspace = workspace_root_from_current_exe();
    let needs_build = !tray.is_file()
        || tray_lock_path()
            .as_ref()
            .is_some_and(|lock| tray_needs_restart(tray, lock))
        || workspace
            .as_ref()
            .is_some_and(|root| dev_tray_sources_newer_than_binary(tray, root));
    if !needs_build {
        return;
    }
    let Some(workspace) = workspace else {
        tracing::warn!(
            "walls-tray missing at {}; run `cargo build -p walls-tray` from the repo",
            tray.display()
        );
        return;
    };

    tracing::info!("building walls-tray for dev ({})", tray.display());
    let status = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("walls-tray")
        .current_dir(&workspace)
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => tracing::warn!("walls-tray build failed with status {s}"),
        Err(err) => tracing::warn!("walls-tray build failed: {err}"),
    }
}

fn tray_is_dev_target(tray: &Path) -> bool {
    tray.components().any(|part| part.as_os_str() == "target")
}

/// True when `crates/tray` or `crates/core` sources are newer than the tray binary.
fn dev_tray_sources_newer_than_binary(tray: &Path, workspace: &Path) -> bool {
    let Some(tray_mtime) = file_modified(tray) else {
        return true;
    };
    let sources = [
        workspace.join("crates/tray/src"),
        workspace.join("crates/core/src"),
    ];
    sources
        .iter()
        .any(|dir| newest_source_mtime(dir).is_some_and(|source_mtime| source_mtime > tray_mtime))
}

fn newest_source_mtime(dir: &Path) -> Option<SystemTime> {
    if !dir.is_dir() {
        return None;
    }
    let mut stack = vec![dir.to_path_buf()];
    let mut newest: Option<SystemTime> = None;
    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            if let Ok(mtime) = entry.metadata().and_then(|meta| meta.modified()) {
                newest = Some(match newest {
                    Some(current) => current.max(mtime),
                    None => mtime,
                });
            }
        }
    }
    newest
}

fn workspace_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?; // profile dir (e.g. debug)
    dir = dir.parent()?; // target
    let root = dir.parent()?; // workspace root
    root.join("Cargo.toml")
        .is_file()
        .then(|| root.to_path_buf())
}

fn tray_lock_path() -> Option<PathBuf> {
    WallsPaths::discover()
        .ok()
        .map(|paths| paths.config_dir.join("tray.lock"))
}

fn tray_pid_path(lock_path: &Path) -> PathBuf {
    lock_path.with_file_name("tray.pid")
}

fn tray_is_running() -> bool {
    let Some(lock_path) = tray_lock_path() else {
        return false;
    };
    let pid_path = tray_pid_path(&lock_path);
    let Ok(pid_str) = fs::read_to_string(&pid_path) else {
        return false;
    };
    let Ok(pid) = pid_str.trim().parse::<u32>() else {
        return false;
    };
    proc_exists(pid) && is_walls_tray_process(pid)
}

fn is_walls_tray_process(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(exe) = fs::read_link(format!("/proc/{pid}/exe")) {
            exe.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "walls-tray")
        } else {
            false
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        proc_exists(pid)
    }
}

fn tray_needs_restart(tray: &Path, lock_path: &Path) -> bool {
    if !tray.is_file() {
        return false;
    }

    if file_modified(tray)
        .zip(file_modified(lock_path))
        .is_some_and(|(tray_mtime, lock_mtime)| tray_mtime > lock_mtime)
    {
        return true;
    }

    running_tray_exe(lock_path)
        .and_then(|running| {
            let want = tray.canonicalize().ok()?;
            let got = running.canonicalize().ok()?;
            Some(want != got)
        })
        .unwrap_or(false)
}

fn file_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

fn running_tray_exe(lock_path: &Path) -> Option<PathBuf> {
    let pid = fs::read_to_string(tray_pid_path(lock_path))
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()?;
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

fn proc_exists(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

/// Detach a child from the TUI terminal session so it survives quitting/closing the launcher tab.
#[cfg(unix)]
fn configure_detached_spawn(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_detached_spawn(_cmd: &mut Command) {}

fn spawn_tray() -> EnsureTrayOutcome {
    let tray = resolve_tray_bin();
    if tray.parent().is_some() && !tray.is_file() {
        let reason = format!(
            "walls-tray not found at {}; run `cargo build -p walls-tray` or set WALLS_TRAY_BIN",
            tray.display()
        );
        tracing::warn!("{reason}");
        return EnsureTrayOutcome::Failed { reason };
    }

    use std::process::Stdio;

    let mut cmd = Command::new(&tray);
    if let Ok(walls) = std::env::current_exe() {
        cmd.env("WALLS_BIN", &walls);
    }
    // Tray logs must not inherit the TUI terminal — they corrupt the alternate screen.
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_detached_spawn(&mut cmd);

    match cmd.spawn() {
        Ok(_) => {
            tracing::info!("spawned walls-tray ({})", tray.display());
            EnsureTrayOutcome::Spawned
        }
        Err(err) => {
            let reason = format!("failed to start walls-tray ({}): {err}", tray.display());
            tracing::warn!("{reason}");
            EnsureTrayOutcome::Failed { reason }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tray_bin_env_takes_precedence() {
        let path = resolve_tray_bin_from(Some("/custom/walls-tray"), None);
        assert_eq!(path, PathBuf::from("/custom/walls-tray"));
    }

    #[test]
    fn uses_sibling_tray_next_to_current_walls_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls");
        fs::write(&exe, b"").unwrap();
        let tray = dir.path().join("walls-tray");
        fs::write(&tray, b"").unwrap();

        let path = resolve_tray_bin_from(None, Some(&exe));
        assert_eq!(path, tray);
    }

    #[test]
    fn falls_back_when_exe_has_no_parent() {
        let path = resolve_tray_bin_from(None, Some(Path::new("walls")));
        assert_eq!(path, PathBuf::from("walls-tray"));
    }

    #[test]
    fn falls_back_when_sibling_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls");
        fs::write(&exe, b"").unwrap();

        let path = resolve_tray_bin_from(None, Some(&exe));
        assert_eq!(path, PathBuf::from("walls-tray"));
    }

    #[test]
    fn skipped_outcome_exposes_tui_message() {
        let outcome = EnsureTrayOutcome::Skipped {
            reason: "COSMIC has no tray".into(),
        };
        assert_eq!(
            outcome.tui_message(),
            Some("tray: COSMIC has no tray".into())
        );
    }

    #[test]
    fn already_running_defers_rotation_without_tui_message() {
        let outcome = EnsureTrayOutcome::AlreadyRunning;
        assert!(outcome.owns_auto_rotation());
        assert_eq!(outcome.tui_message(), None);
    }

    #[test]
    fn tray_needs_restart_when_binary_is_newer_than_lock() {
        let dir = tempfile::tempdir().unwrap();
        let tray = dir.path().join("walls-tray");
        let lock = dir.path().join("tray.lock");
        fs::write(&lock, b"").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&tray, b"tray").unwrap();

        assert!(tray_needs_restart(&tray, &lock));
    }

    #[test]
    fn tray_does_not_need_restart_when_lock_is_newer() {
        let dir = tempfile::tempdir().unwrap();
        let tray = dir.path().join("walls-tray");
        let lock = dir.path().join("tray.lock");
        fs::write(&tray, b"tray").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&lock, b"").unwrap();

        assert!(!tray_needs_restart(&tray, &lock));
    }
}

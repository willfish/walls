mod actions;
pub mod appindicator;
mod bin;
pub mod icon;
pub mod lock;
pub mod platform;
mod rotation;
pub mod sni;
mod state_watch;
pub mod tui;

use std::path::PathBuf;
use std::process::Command;

pub use bin::resolve_walls_bin;

#[derive(Debug, Clone, Copy)]
pub enum WallsCommand {
    Next,
    Prev,
    TogglePause,
}

impl WallsCommand {
    pub fn args(self) -> &'static [&'static str] {
        match self {
            Self::Next => &["next"],
            Self::Prev => &["prev"],
            Self::TogglePause => &["toggle-pause"],
        }
    }
}

pub fn run_walls(walls: &PathBuf, args: &[&str]) -> anyhow::Result<()> {
    use std::process::Stdio;

    // Never write to the terminal — avoids corrupting a running walls TUI.
    let status = Command::new(walls)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        anyhow::bail!("{} {} failed: {status}", walls.display(), args.join(" "));
    }
    Ok(())
}

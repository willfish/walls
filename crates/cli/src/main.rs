use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{prelude::*, EnvFilter, Layer};
use walls_core::apply::ApplyTrigger;
use walls_core::{RefreshLevel, WallsCtx};

#[cfg(feature = "tui")]
mod tui;

mod bin_utils;

#[derive(Parser)]
#[command(name = "walls", version, about = "Wallpaper manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set a local image as the wallpaper
    Apply { path: PathBuf },
    /// Show next wallpaper from configured sources
    Next {
        /// User-initiated next (ignores pause and rotation disabled)
        #[arg(long, short = 'm')]
        manual: bool,
        /// Refresh current wallpaper instead of selecting a new one
        #[arg(long, value_enum)]
        refresh: Option<CliRefreshLevel>,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Show previous wallpaper from history
    Prev {
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Print status
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Pause automatic changes
    Pause,
    /// Resume automatic changes
    Resume,
    /// Toggle pause state
    TogglePause,
    /// Print the current wallpaper path
    Current {
        /// Emit the current wallpaper metadata JSON. Prefer --json for scripts that need a stable envelope.
        #[arg(long, conflicts_with = "json")]
        meta: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Copy the current wallpaper into the favorites folder
    Favorite,
    /// Import images into the fetched folder
    Fetch {
        paths: Vec<PathBuf>,
        #[arg(long)]
        r#move: bool,
    },
    /// Delete the current wallpaper from disk and state
    Trash,
    /// Interactive terminal UI
    #[cfg(feature = "tui")]
    Tui,
    /// Config file utilities
    Config {
        #[command(subcommand)]
        sub: ConfigSub,
    },
}

#[derive(Subcommand)]
enum ConfigSub {
    /// Validate config.json and secrets
    Validate {
        /// Emit structured validation diagnostics as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reconcile derived config artifacts (tray autostart)
    Sync,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliRefreshLevel {
    All,
    FiltersAndTexts,
    Texts,
    ClockOnly,
}

impl From<CliRefreshLevel> for RefreshLevel {
    fn from(value: CliRefreshLevel) -> Self {
        match value {
            CliRefreshLevel::All => Self::All,
            CliRefreshLevel::FiltersAndTexts => Self::FiltersAndTexts,
            CliRefreshLevel::Texts => Self::Texts,
            CliRefreshLevel::ClockOnly => Self::ClockOnly,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("walls=info".parse()?);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(tui::ConsoleWriter)
                .with_ansi(true)
                .with_filter(filter.clone()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(tui::CaptureWriter)
                .with_ansi(false)
                .with_filter(filter),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Apply { path }) => cmd_apply(path)?,
        Some(Command::Next {
            manual,
            refresh,
            json,
        }) => cmd_next(manual, refresh, json).await?,
        Some(Command::Prev { json }) => cmd_prev(json)?,
        Some(Command::Status { json }) => cmd_status(json)?,
        Some(Command::Pause) => cmd_pause(true)?,
        Some(Command::Resume) => cmd_pause(false)?,
        Some(Command::TogglePause) => cmd_toggle_pause()?,
        Some(Command::Current { meta, json }) => cmd_current(meta, json)?,
        Some(Command::Favorite) => cmd_favorite()?,
        Some(Command::Fetch { paths, r#move }) => cmd_fetch(paths, r#move)?,
        Some(Command::Trash) => cmd_trash()?,
        Some(Command::Config { sub }) => match sub {
            ConfigSub::Validate { json } => cmd_config_validate(json)?,
            ConfigSub::Sync => cmd_config_sync()?,
        },
        #[cfg(feature = "tui")]
        Some(Command::Tui) | None => {
            let tray = bin_utils::ensure_tray_running();
            return tui::run(tray.tui_message(), tray.owns_auto_rotation()).context("tui failed");
        }
        #[cfg(not(feature = "tui"))]
        None => {
            eprintln!("walls: no command specified (try `walls apply <path>`)");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_apply(path: PathBuf) -> anyhow::Result<()> {
    let path = walls_core::expand_home(&path);
    let mut ctx = WallsCtx::load()?;
    ctx.apply_file(&path, ApplyTrigger::Manual)?;
    println!("{}", path.display());
    Ok(())
}

fn cmd_status(json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "paused": ctx.state.paused,
                "current": ctx.state.current,
                "history_len": ctx.state.history.len(),
                "cache_queue_len": ctx.state.cache_queue.len(),
            }))?
        );
    } else {
        println!("paused: {}", ctx.state.paused);
        if let Some(cur) = &ctx.state.current {
            println!("current: {}", cur.composed_path);
        } else {
            println!("current: (none)");
        }
        println!("history: {} entries", ctx.state.history.len());
        println!("cache queue: {} entries", ctx.state.cache_queue.len());
    }
    Ok(())
}

fn print_json(value: serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    std::io::stdout().flush()?;
    Ok(())
}

fn command_result(
    command: &str,
    changed: bool,
    status: &str,
    path: Option<PathBuf>,
    exit_code_reason: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "command": command,
        "changed": changed,
        "status": status,
        "path": path.map(|path| path.display().to_string()),
        "exit_code_reason": exit_code_reason,
    })
}

async fn cmd_next(
    manual: bool,
    refresh: Option<CliRefreshLevel>,
    json: bool,
) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    if let Some(level) = refresh {
        match ctx.refresh_current(level.into())? {
            Some(p) if json => {
                print_json(command_result("next", true, "refreshed", Some(p), None))?
            }
            Some(p) => println!("{}", p.display()),
            None if json => print_json(command_result(
                "next",
                false,
                "missing_current",
                None,
                Some("missing_current"),
            ))?,
            None => println!("no current wallpaper"),
        }
        return Ok(());
    }
    let applied = if manual {
        ctx.advance_next_manual().await?
    } else {
        ctx.advance_next().await?
    };
    match applied {
        Some(p) if json => print_json(command_result("next", true, "applied", Some(p), None))?,
        Some(p) => println!("{}", p.display()),
        None if json => print_json(command_result(
            "next",
            false,
            "no_change",
            None,
            Some("no_change"),
        ))?,
        None => println!("no change"),
    }
    Ok(())
}

fn cmd_prev(json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_prev()? {
        Some(p) if json => print_json(command_result(
            "prev",
            true,
            "applied_previous",
            Some(p),
            None,
        ))?,
        Some(p) => println!("{}", p.display()),
        None if json => print_json(command_result(
            "prev",
            false,
            "no_previous",
            None,
            Some("no_previous"),
        ))?,
        None => println!("no previous"),
    }
    Ok(())
}

fn cmd_pause(paused: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    ctx.set_paused(paused)?;
    println!("paused: {paused}");
    Ok(())
}

fn cmd_toggle_pause() -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    ctx.toggle_pause()?;
    println!("paused: {}", ctx.state.paused);
    Ok(())
}

fn cmd_config_validate(json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let diagnostics =
        walls_core::validate::validate_config_diagnostics(&ctx.config, &ctx.secrets, &ctx.paths);
    if json {
        println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    } else if diagnostics.is_empty() {
        println!("config ok");
        return Ok(());
    } else {
        for diagnostic in &diagnostics {
            eprintln!("{diagnostic}");
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn cmd_config_sync() -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    match walls_core::autostart::sync_tray_autostart(&ctx.config)? {
        walls_core::autostart::AutostartSyncOutcome::Written => {
            println!("tray autostart: updated");
        }
        walls_core::autostart::AutostartSyncOutcome::Removed => {
            println!("tray autostart: removed");
        }
        walls_core::autostart::AutostartSyncOutcome::Skipped { reason } => {
            println!("tray autostart: skipped ({reason})");
        }
    }
    Ok(())
}

fn cmd_trash() -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    ctx.trash_current()?;
    println!("trashed");
    Ok(())
}

fn cmd_fetch(paths: Vec<PathBuf>, move_files: bool) -> anyhow::Result<()> {
    if paths.is_empty() {
        anyhow::bail!("fetch requires at least one path");
    }
    let ctx = WallsCtx::load()?;
    for dest in ctx.fetch_files(&paths, move_files)? {
        println!("{}", dest.display());
    }
    Ok(())
}

fn cmd_favorite() -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let dest = ctx.favorite_current()?;
    println!("{}", dest.display());
    Ok(())
}

fn cmd_current(meta: bool, json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    if meta {
        let value = ctx
            .current_meta()
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if json {
        match ctx.current_path() {
            Some(path) => {
                return print_json(serde_json::json!({
                    "command": "current",
                    "changed": false,
                    "status": "current",
                    "current": {
                        "path": path.display().to_string(),
                        "meta": ctx.current_meta().map(serde_json::to_value).transpose()?,
                    },
                    "exit_code_reason": null,
                }));
            }
            None => {
                print_json(serde_json::json!({
                    "command": "current",
                    "changed": false,
                    "status": "missing_current",
                    "current": null,
                    "exit_code_reason": "missing_current",
                }))?;
                std::process::exit(1);
            }
        }
    }
    match ctx.current_path() {
        Some(p) => println!("{}", p.display()),
        None => {
            println!("(none)");
            std::process::exit(1);
        }
    }
    Ok(())
}

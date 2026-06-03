use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use walls_core::apply::ApplyTrigger;
use walls_core::WallsCtx;

#[cfg(feature = "tui")]
mod tui;

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
    Next,
    /// Show previous wallpaper from history
    Prev,
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
        #[arg(long)]
        meta: bool,
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
    Validate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walls=info".parse()?))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Some(Command::Apply { path }) => cmd_apply(path)?,
        Some(Command::Next) => cmd_next().await?,
        Some(Command::Prev) => cmd_prev()?,
        Some(Command::Status { json }) => cmd_status(json)?,
        Some(Command::Pause) => cmd_pause(true)?,
        Some(Command::Resume) => cmd_pause(false)?,
        Some(Command::TogglePause) => cmd_toggle_pause()?,
        Some(Command::Current { meta }) => cmd_current(meta)?,
        Some(Command::Favorite) => cmd_favorite()?,
        Some(Command::Fetch { paths, r#move }) => cmd_fetch(paths, r#move)?,
        Some(Command::Trash) => cmd_trash()?,
        Some(Command::Config { sub }) => match sub {
            ConfigSub::Validate => cmd_config_validate()?,
        },
        #[cfg(feature = "tui")]
        Some(Command::Tui) => return tui::run().context("tui failed"),
        None => {
            #[cfg(feature = "tui")]
            if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                return tui::run().context("tui failed");
            }
            eprintln!("walls: no command specified (try `walls apply <path>` or `walls tui`)");
            std::process::exit(1);
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

async fn cmd_next() -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_next().await? {
        Some(p) => println!("{}", p.display()),
        None => println!("no change"),
    }
    Ok(())
}

fn cmd_prev() -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_prev()? {
        Some(p) => println!("{}", p.display()),
        None => println!("no previous"),
    }
    Ok(())
}

fn cmd_pause(paused: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    if ctx.state.paused != paused {
        ctx.state.paused = paused;
        ctx.save_state()?;
    }
    println!("paused: {paused}");
    Ok(())
}

fn cmd_toggle_pause() -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    ctx.toggle_pause()?;
    println!("paused: {}", ctx.state.paused);
    Ok(())
}

fn cmd_config_validate() -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let errors = walls_core::validate::validate_config(&ctx.config, &ctx.secrets, &ctx.paths);
    if errors.is_empty() {
        println!("config ok");
        return Ok(());
    }
    for err in &errors {
        eprintln!("error: {err}");
    }
    std::process::exit(1);
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

fn cmd_current(meta: bool) -> anyhow::Result<()> {
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
    match ctx.current_path() {
        Some(p) => println!("{}", p.display()),
        None => {
            println!("(none)");
            std::process::exit(1);
        }
    }
    Ok(())
}

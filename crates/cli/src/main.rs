use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{prelude::*, EnvFilter, Layer};
use walls_core::apply::ApplyTrigger;
use walls_core::apply::{backend_setting_label, desktop_display_name, summarize_apply_environment};
use walls_core::doctor::{DoctorOptions, DoctorReport, DoctorSection, DoctorStatus};
use walls_core::downloads::{NukeDownloadsMode, NukeDownloadsPlan};
use walls_core::events::{last_run_summary, read_events, EventKind, EventRecord, LastRunSummary};
use walls_core::providers::{
    ProviderAttempt, ProviderAttemptOutcome, ProviderFailureKind, ProviderKind,
    ProviderNoCandidateReason, ProviderOperation, ProviderRetryReason, ProviderStatus,
    ProviderStatusReport,
};
use walls_core::{RefreshLevel, WallsCtx, WallsError};

#[cfg(feature = "tui")]
mod tui;

mod bin_utils;
mod recovery;

#[derive(Parser)]
#[command(name = "walls", version, about = "Wallpaper manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set a local image as the wallpaper
    Apply {
        path: PathBuf,
        /// Show what would be applied without composing, running the backend, or mutating state
        #[arg(long)]
        dry_run: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Show next wallpaper from configured sources
    Next {
        /// User-initiated next (ignores pause and rotation disabled)
        #[arg(long, short = 'm')]
        manual: bool,
        /// Refresh current wallpaper instead of selecting a new one
        #[arg(long, value_enum)]
        refresh: Option<CliRefreshLevel>,
        /// Show what would be considered without fetching, applying, or mutating state
        #[arg(long, conflicts_with = "refresh")]
        dry_run: bool,
        /// Print provider attempts, retries, skips, failures, and fallbacks
        #[arg(long, short = 'v')]
        verbose: bool,
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
    /// Restore the previous wallpaper from history
    Undo {
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Print status
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Inspect recent wallpaper, provider, and apply activity
    Logs {
        /// Number of recent matching events to show
        #[arg(long, default_value_t = 20)]
        tail: usize,
        /// Restrict to a provider id or kind, such as local, wallhaven, or unsplash
        #[arg(long)]
        provider: Option<String>,
        /// Restrict to derived event severity
        #[arg(long, value_enum)]
        level: Option<CliLogLevel>,
        /// Restrict to events at or after this Unix timestamp
        #[arg(long)]
        since: Option<u64>,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Check whether walls is ready for this machine
    Doctor {
        /// Emit stable machine-readable diagnostic checks as JSON
        #[arg(long)]
        json: bool,
    },
    /// Inspect and prune cache, queue, downloaded provider files, and quota usage
    Cache {
        #[command(subcommand)]
        sub: CacheSub,
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
    Trash {
        /// Show what would be removed without mutating state or files
        #[arg(long)]
        dry_run: bool,
        /// Required to remove files and mutate state
        #[arg(long)]
        force: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
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
    Sync {
        /// Show what would change without writing or removing autostart files
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum CacheSub {
    /// Summarise cache directories, queue, and quota usage
    Status {
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// List provider cache/downloaded files
    Inspect {
        /// Restrict output to a provider such as wallhaven, unsplash, reddit, or downloaded
        #[arg(long)]
        provider: Option<String>,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Clear the queue first, or purge provider files when the queue is empty
    Prune {
        /// Show what would change without mutating state or files
        #[arg(long)]
        dry_run: bool,
        /// Required to mutate state or remove files
        #[arg(long)]
        force: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Clear queued provider downloads
    ClearQueue {
        /// Show what would change without mutating state
        #[arg(long)]
        dry_run: bool,
        /// Required to mutate state
        #[arg(long)]
        force: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
    /// Remove provider cache files and downloaded provider artifacts
    PurgeProviderFiles {
        /// Show what would change without removing files or mutating state
        #[arg(long)]
        dry_run: bool,
        /// Required to remove files and mutate state
        #[arg(long)]
        force: bool,
        /// Emit a stable JSON command result
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliRefreshLevel {
    All,
    FiltersAndTexts,
    Texts,
    ClockOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliLogLevel {
    Info,
    Warn,
    Error,
}

impl CliLogLevel {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
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
        Some(Command::Apply {
            path,
            dry_run,
            json,
        }) => cmd_apply(path, dry_run, json)?,
        Some(Command::Next {
            manual,
            refresh,
            dry_run,
            verbose,
            json,
        }) => cmd_next(manual, refresh, dry_run, verbose, json).await?,
        Some(Command::Prev { json }) => cmd_prev(json)?,
        Some(Command::Undo { json }) => cmd_undo(json)?,
        Some(Command::Status { json }) => cmd_status(json)?,
        Some(Command::Logs {
            tail,
            provider,
            level,
            since,
            json,
        }) => cmd_logs(tail, provider, level, since, json)?,
        Some(Command::Doctor { json }) => cmd_doctor(json)?,
        Some(Command::Cache { sub }) => match sub {
            CacheSub::Status { json } => cmd_cache_status(json)?,
            CacheSub::Inspect { provider, json } => cmd_cache_inspect(provider, json)?,
            CacheSub::Prune {
                dry_run,
                force,
                json,
            } => cmd_cache_prune(dry_run, force, json)?,
            CacheSub::ClearQueue {
                dry_run,
                force,
                json,
            } => cmd_cache_clear_queue(dry_run, force, json)?,
            CacheSub::PurgeProviderFiles {
                dry_run,
                force,
                json,
            } => cmd_cache_purge_provider_files(dry_run, force, json)?,
        },
        Some(Command::Pause) => cmd_pause(true)?,
        Some(Command::Resume) => cmd_pause(false)?,
        Some(Command::TogglePause) => cmd_toggle_pause()?,
        Some(Command::Current { meta, json }) => cmd_current(meta, json)?,
        Some(Command::Favorite) => cmd_favorite()?,
        Some(Command::Fetch { paths, r#move }) => cmd_fetch(paths, r#move)?,
        Some(Command::Trash {
            dry_run,
            force,
            json,
        }) => cmd_trash(dry_run, force, json)?,
        Some(Command::Config { sub }) => match sub {
            ConfigSub::Validate { json } => cmd_config_validate(json)?,
            ConfigSub::Sync { dry_run } => cmd_config_sync(dry_run)?,
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

fn cmd_apply(path: PathBuf, dry_run: bool, json: bool) -> anyhow::Result<()> {
    let path = walls_core::expand_home(&path);
    if dry_run {
        let ctx = WallsCtx::load_without_autostart_sync()?;
        let missing_original = !path.exists();
        let plan = apply_dry_run_json(&ctx, &path);
        print_json_or_human(json, plan, || {
            if missing_original {
                println!(
                    "would fail: wallpaper file does not exist: {}",
                    path.display()
                );
            } else {
                println!("would apply original: {}", path.display());
                println!(
                    "would run backend: {}",
                    backend_setting_label(walls_core::apply::resolved_apply_backend(
                        &ctx.config.apply
                    ))
                );
                println!("would update current wallpaper state");
                println!("would move this wallpaper to the front of history");
                println!("would record an apply event");
            }
            println!("no wallpaper files, state, history, or event journal were changed");
            Ok(())
        })?;
        if missing_original {
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut ctx = WallsCtx::load()?;
    ctx.apply_file(&path, ApplyTrigger::Manual)?;
    if json {
        print_json(serde_json::json!({
            "command": "apply",
            "changed": true,
            "status": "applied",
            "dry_run": false,
            "apply": {
                "original_path": path,
            },
            "exit_code_reason": null,
        }))?;
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn apply_dry_run_json(ctx: &WallsCtx, path: &std::path::Path) -> serde_json::Value {
    let summary = summarize_apply_environment(&ctx.config.apply);
    let original_exists = path.exists();
    serde_json::json!({
        "command": "apply",
        "changed": false,
        "status": if original_exists { "would_apply" } else { "missing_original" },
        "dry_run": true,
        "apply": {
            "original_path": path,
            "original_exists": original_exists,
            "configured_backend": backend_setting_label(summary.configured_backend),
            "resolved_backend": backend_setting_label(summary.resolved_backend),
            "display_mode": ctx.config.display.mode,
            "compose_dir": ctx.paths.compose_dir,
            "would_compose": ctx.config.display.auto_rotate
                || ctx.config.display.filters.enabled
                || (ctx.config.display.target_width.is_some()
                    && ctx.config.display.target_height.is_some()),
            "would_run_backend": original_exists,
            "would_update_current": original_exists,
            "would_update_history": original_exists,
            "would_record_event": original_exists,
        },
        "exit_code_reason": if original_exists {
            serde_json::Value::Null
        } else {
            serde_json::Value::String("missing_original".into())
        },
    })
}

fn cmd_status(json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let last_run = read_last_run_summary(&ctx)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "paused": ctx.state.paused,
                "current": ctx.state.current,
                "history_len": ctx.state.history.len(),
                "cache_queue_len": ctx.state.cache_queue.len(),
                "desktop": desktop_status_json(&ctx),
                "last_run": last_run,
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
        print_last_run_human(last_run.as_ref());
    }
    Ok(())
}

fn read_last_run_summary(ctx: &WallsCtx) -> anyhow::Result<Option<LastRunSummary>> {
    let events = read_events(&ctx.paths.event_journal_file)
        .with_context(|| format!("read {}", ctx.paths.event_journal_file.display()))?;
    Ok(last_run_summary(&events))
}

fn print_last_run_human(last_run: Option<&LastRunSummary>) {
    let Some(last_run) = last_run else {
        println!("last run: (none)");
        return;
    };
    println!(
        "last run: {} at {}",
        last_run.message, last_run.timestamp_unix
    );
    if let Some(path) = &last_run.applied_path {
        println!("last applied: {path}");
    }
    if let Some(reason) = &last_run.no_change_reason {
        println!("last no-change reason: {reason}");
    }
    for warning in &last_run.warnings {
        println!("last warning: {warning}");
    }
    for error in &last_run.errors {
        println!("last error: {error}");
    }
}

fn cmd_logs(
    tail: usize,
    provider: Option<String>,
    level: Option<CliLogLevel>,
    since: Option<u64>,
    json: bool,
) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let mut events = read_events(&ctx.paths.event_journal_file)
        .with_context(|| format!("read {}", ctx.paths.event_journal_file.display()))?;
    events.retain(|event| log_event_matches(event, provider.as_deref(), level, since));
    events.reverse();
    events.truncate(tail);

    if json {
        return print_json(logs_json(
            &ctx,
            &events,
            tail,
            provider.as_deref(),
            level,
            since,
        )?);
    }
    if events.is_empty() {
        println!(
            "no log events. Run `walls next --manual` or `walls apply <path>` to create activity."
        );
        return Ok(());
    }
    for event in &events {
        println!("{}", log_event_line(event));
    }
    Ok(())
}

fn log_event_matches(
    event: &EventRecord,
    provider: Option<&str>,
    level: Option<CliLogLevel>,
    since: Option<u64>,
) -> bool {
    if since.is_some_and(|since| event.timestamp_unix < since) {
        return false;
    }
    if level.is_some_and(|level| log_event_level(event) != level) {
        return false;
    }
    if let Some(provider) = provider {
        let provider = provider.to_ascii_lowercase();
        return log_event_provider_labels(event)
            .into_iter()
            .any(|label| label == provider);
    }
    true
}

fn log_event_provider_labels(event: &EventRecord) -> Vec<String> {
    match &event.kind {
        EventKind::Apply {
            provider: Some(provider),
            ..
        } => vec![provider.to_ascii_lowercase()],
        EventKind::Apply { provider: None, .. } => vec!["local".into()],
        EventKind::ApplyFailed {
            provider: Some(provider),
            ..
        } => vec![provider.to_ascii_lowercase()],
        EventKind::ApplyFailed { provider: None, .. } => vec!["local".into()],
        EventKind::ProviderAttempt { attempt } => vec![
            attempt.provider_id.to_ascii_lowercase(),
            provider_kind_label(attempt.provider_kind).to_string(),
        ],
    }
}

fn log_event_level(event: &EventRecord) -> CliLogLevel {
    match &event.kind {
        EventKind::Apply { .. } => CliLogLevel::Info,
        EventKind::ApplyFailed { .. } => CliLogLevel::Error,
        EventKind::ProviderAttempt { attempt } => match &attempt.outcome {
            ProviderAttemptOutcome::Failed { .. } => CliLogLevel::Error,
            ProviderAttemptOutcome::Skipped { .. }
            | ProviderAttemptOutcome::NoCandidates { .. } => CliLogLevel::Warn,
            ProviderAttemptOutcome::NotRun | ProviderAttemptOutcome::Applied { .. } => {
                CliLogLevel::Info
            }
        },
    }
}

fn log_event_line(event: &EventRecord) -> String {
    match &event.kind {
        EventKind::Apply {
            trigger,
            original_path,
            composed_path,
            provider,
        } => format!(
            "{}\t{}\tapply\ttrigger={}\tprovider={}\toriginal={}\tcomposed={}",
            event.timestamp_unix,
            log_event_level(event).label(),
            apply_trigger_label(*trigger),
            provider.as_deref().unwrap_or("local"),
            original_path,
            composed_path
        ),
        EventKind::ApplyFailed {
            trigger,
            original_path,
            composed_path,
            provider,
            message,
        } => format!(
            "{}\t{}\tapply_failed\ttrigger={}\tprovider={}\toriginal={}\tcomposed={}\terror={}",
            event.timestamp_unix,
            log_event_level(event).label(),
            apply_trigger_label(*trigger),
            provider.as_deref().unwrap_or("local"),
            original_path,
            composed_path.as_deref().unwrap_or("(none)"),
            message
        ),
        EventKind::ProviderAttempt { attempt } => format!(
            "{}\t{}\tprovider\t{}",
            event.timestamp_unix,
            log_event_level(event).label(),
            provider_attempt_line(attempt)
        ),
    }
}

fn logs_json(
    ctx: &WallsCtx,
    events: &[EventRecord],
    tail: usize,
    provider: Option<&str>,
    level: Option<CliLogLevel>,
    since: Option<u64>,
) -> anyhow::Result<serde_json::Value> {
    let events = events
        .iter()
        .map(log_event_json)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(serde_json::json!({
        "command": "logs",
        "changed": false,
        "status": if events.is_empty() { "empty" } else { "ok" },
        "journal": ctx.paths.event_journal_file.display().to_string(),
        "filters": {
            "tail": tail,
            "provider": provider,
            "level": level.map(CliLogLevel::label),
            "since": since,
        },
        "events": events,
        "exit_code_reason": null,
    }))
}

fn log_event_json(event: &EventRecord) -> anyhow::Result<serde_json::Value> {
    let mut value = serde_json::to_value(event)?;
    if let serde_json::Value::Object(fields) = &mut value {
        fields.insert(
            "level".into(),
            serde_json::Value::String(log_event_level(event).label().into()),
        );
        fields.insert(
            "message".into(),
            serde_json::Value::String(log_event_message(event)),
        );
    }
    Ok(value)
}

fn log_event_message(event: &EventRecord) -> String {
    match &event.kind {
        EventKind::Apply {
            trigger,
            original_path,
            ..
        } => format!(
            "applied {} wallpaper from {}",
            apply_trigger_label(*trigger),
            original_path
        ),
        EventKind::ApplyFailed {
            trigger, message, ..
        } => format!(
            "failed to apply {} wallpaper: {}",
            apply_trigger_label(*trigger),
            message
        ),
        EventKind::ProviderAttempt { attempt } => provider_attempt_line(attempt),
    }
}

fn apply_trigger_label(trigger: ApplyTrigger) -> &'static str {
    match trigger {
        ApplyTrigger::Manual => "manual",
        ApplyTrigger::Auto => "auto",
        ApplyTrigger::Refresh => "refresh",
    }
}

fn cmd_cache_status(json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let report = cache_status_json(&ctx);
    if json {
        print_json(report)?;
        return Ok(());
    }
    let inspection = ctx.inspect_cache();
    println!("cache dir: {}", ctx.paths.cache_dir.display());
    println!(
        "cache files: {} provider / {} total ({} bytes provider / {} bytes total)",
        inspection.cache.provider_files,
        inspection.cache.files,
        inspection.cache.provider_bytes,
        inspection.cache.bytes
    );
    println!("download dir: {}", ctx.paths.download_dir.display());
    println!(
        "download files: {} files ({} bytes)",
        inspection.downloads.files, inspection.downloads.bytes
    );
    println!("queue: {} entries", inspection.queue_len);
    println!(
        "quota: {} ({} MiB, {} bytes used{})",
        if ctx.config.quota.enabled {
            "enabled"
        } else {
            "disabled"
        },
        ctx.config.quota.size_mb,
        inspection.downloads.bytes,
        quota_suffix(&ctx, inspection.downloads.bytes)
    );
    println!(
        "provider state references: current={}, history={}",
        inspection.current_provider_storage, inspection.history_provider_entries
    );
    Ok(())
}

fn cmd_cache_inspect(provider: Option<String>, json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let files = ctx.list_cache_files(provider.as_deref());
    if json {
        print_json(serde_json::json!({
            "command": "cache inspect",
            "changed": false,
            "status": "ok",
            "provider": provider,
            "files": files.iter().map(cache_file_json).collect::<Vec<_>>(),
            "exit_code_reason": null,
        }))?;
        return Ok(());
    }
    if files.is_empty() {
        println!("no provider cache files");
        return Ok(());
    }
    for file in files {
        println!(
            "{}\t{}\t{}\t{}",
            file.area.label(),
            file.provider.as_deref().unwrap_or("unknown"),
            file.bytes,
            file.path.display()
        );
    }
    Ok(())
}

fn cmd_cache_prune(dry_run: bool, force: bool, json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    let plan = ctx.plan_nuke_downloads();
    if dry_run {
        return print_cache_plan("cache prune", &ctx, &plan, true, json);
    }
    require_force(force, json, "cache prune")?;
    let result = ctx.nuke_downloads()?;
    print_cache_result("cache prune", result, json)
}

fn cmd_cache_clear_queue(dry_run: bool, force: bool, json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    let plan = NukeDownloadsPlan {
        mode: if ctx.state.cache_queue.is_empty() {
            NukeDownloadsMode::Nothing
        } else {
            NukeDownloadsMode::ClearQueue
        },
        queue_len: ctx.state.cache_queue.len(),
        cache_files: 0,
        download_files: 0,
    };
    if dry_run {
        return print_cache_plan("cache clear-queue", &ctx, &plan, true, json);
    }
    require_force(force, json, "cache clear-queue")?;
    let cleared = ctx.clear_cache_queue()?;
    print_json_or_human(
        json,
        serde_json::json!({
            "command": "cache clear-queue",
            "changed": cleared > 0,
            "status": if cleared > 0 { "cleared_queue" } else { "noop" },
            "queue_cleared": cleared,
            "exit_code_reason": null,
        }),
        || {
            if cleared > 0 {
                println!("cleared queue: {cleared} entries");
            } else {
                println!("nothing to clear");
            }
            Ok(())
        },
    )
}

fn cmd_cache_purge_provider_files(dry_run: bool, force: bool, json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    let inspection = ctx.inspect_cache();
    let plan = NukeDownloadsPlan {
        mode: if inspection.cache.provider_files == 0 && inspection.downloads.files == 0 {
            NukeDownloadsMode::Nothing
        } else {
            NukeDownloadsMode::PurgeProviderFiles
        },
        queue_len: 0,
        cache_files: inspection.cache.provider_files,
        download_files: inspection.downloads.files,
    };
    if dry_run {
        return print_cache_plan("cache purge-provider-files", &ctx, &plan, true, json);
    }
    require_force(force, json, "cache purge-provider-files")?;
    let result = ctx.purge_provider_files()?;
    print_cache_result("cache purge-provider-files", result, json)
}

fn quota_suffix(ctx: &WallsCtx, bytes: u64) -> String {
    if !ctx.config.quota.enabled {
        return String::new();
    }
    let limit = ctx.config.quota.size_mb.saturating_mul(1024 * 1024);
    if limit == 0 {
        return String::from(", no valid quota limit");
    }
    if bytes > limit {
        format!(", {} bytes over quota", bytes - limit)
    } else {
        format!(", {} bytes remaining", limit - bytes)
    }
}

fn cache_status_json(ctx: &WallsCtx) -> serde_json::Value {
    let inspection = ctx.inspect_cache();
    let quota_bytes = ctx.config.quota.size_mb.saturating_mul(1024 * 1024);
    serde_json::json!({
        "command": "cache status",
        "changed": false,
        "status": "ok",
        "paths": {
            "cache_dir": ctx.paths.cache_dir.display().to_string(),
            "download_dir": ctx.paths.download_dir.display().to_string(),
        },
        "queue": {
            "len": inspection.queue_len,
            "ids": inspection.queue_ids,
        },
        "cache": {
            "files": inspection.cache.files,
            "bytes": inspection.cache.bytes,
            "provider_files": inspection.cache.provider_files,
            "provider_bytes": inspection.cache.provider_bytes,
        },
        "downloads": {
            "files": inspection.downloads.files,
            "bytes": inspection.downloads.bytes,
            "provider_files": inspection.downloads.provider_files,
            "provider_bytes": inspection.downloads.provider_bytes,
        },
        "quota": {
            "enabled": ctx.config.quota.enabled,
            "size_mb": ctx.config.quota.size_mb,
            "size_bytes": quota_bytes,
            "usage_bytes": inspection.downloads.bytes,
            "over_quota": ctx.config.quota.enabled && quota_bytes > 0 && inspection.downloads.bytes > quota_bytes,
        },
        "state_references": {
            "current_provider_storage": inspection.current_provider_storage,
            "history_provider_entries": inspection.history_provider_entries,
        },
        "exit_code_reason": null,
    })
}

fn cache_file_json(file: &walls_core::downloads::CacheFileEntry) -> serde_json::Value {
    serde_json::json!({
        "area": file.area.label(),
        "name": file.name,
        "path": file.path.display().to_string(),
        "bytes": file.bytes,
        "provider": file.provider,
    })
}

fn print_cache_plan(
    command: &str,
    ctx: &WallsCtx,
    plan: &NukeDownloadsPlan,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let status = cache_plan_status(plan, dry_run);
    print_json_or_human(
        json,
        serde_json::json!({
            "command": command,
            "changed": false,
            "status": status,
            "dry_run": dry_run,
            "plan": {
                "mode": plan.mode.label(),
                "queue_len": plan.queue_len,
                "cache_files": plan.cache_files,
                "download_files": plan.download_files,
                "cache_dir": ctx.paths.cache_dir.display().to_string(),
                "download_dir": ctx.paths.download_dir.display().to_string(),
            },
            "exit_code_reason": null,
        }),
        || {
            match plan.mode {
                NukeDownloadsMode::ClearQueue => {
                    println!("would clear queue: {} entries", plan.queue_len);
                }
                NukeDownloadsMode::PurgeProviderFiles => {
                    println!(
                        "would purge provider files: {} cache files, {} downloaded files",
                        plan.cache_files, plan.download_files
                    );
                }
                NukeDownloadsMode::Nothing => println!("nothing to prune"),
            }
            Ok(())
        },
    )
}

fn print_cache_result(
    command: &str,
    result: walls_core::downloads::NukeDownloadsResult,
    json: bool,
) -> anyhow::Result<()> {
    let changed =
        result.queue_cleared > 0 || result.cache_removed > 0 || result.download_removed > 0;
    print_json_or_human(
        json,
        serde_json::json!({
            "command": command,
            "changed": changed,
            "status": cache_result_status(&result),
            "mode": result.mode.label(),
            "queue_cleared": result.queue_cleared,
            "cache_removed": result.cache_removed,
            "download_removed": result.download_removed,
            "exit_code_reason": null,
        }),
        || {
            match result.mode {
                NukeDownloadsMode::ClearQueue => {
                    println!("cleared queue: {} entries", result.queue_cleared);
                }
                NukeDownloadsMode::PurgeProviderFiles => {
                    println!(
                        "purged provider files: {} cache files, {} downloaded files",
                        result.cache_removed, result.download_removed
                    );
                }
                NukeDownloadsMode::Nothing => println!("nothing to prune"),
            }
            Ok(())
        },
    )
}

fn print_json_or_human(
    json: bool,
    value: serde_json::Value,
    human: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if json {
        print_json(value)
    } else {
        human()
    }
}

fn cache_plan_status(plan: &NukeDownloadsPlan, dry_run: bool) -> &'static str {
    match (dry_run, plan.mode) {
        (_, NukeDownloadsMode::Nothing) => "noop",
        (true, NukeDownloadsMode::ClearQueue) => "would_clear_queue",
        (true, NukeDownloadsMode::PurgeProviderFiles) => "would_purge_provider_files",
        (false, NukeDownloadsMode::ClearQueue) => "clear_queue",
        (false, NukeDownloadsMode::PurgeProviderFiles) => "purge_provider_files",
    }
}

fn cache_result_status(result: &walls_core::downloads::NukeDownloadsResult) -> &'static str {
    match result.mode {
        NukeDownloadsMode::ClearQueue if result.queue_cleared > 0 => "cleared_queue",
        NukeDownloadsMode::PurgeProviderFiles
            if result.cache_removed > 0 || result.download_removed > 0 =>
        {
            "purged_provider_files"
        }
        _ => "noop",
    }
}

fn require_force(force: bool, json: bool, command: &str) -> anyhow::Result<()> {
    if force {
        return Ok(());
    }
    if json {
        print_json(serde_json::json!({
            "command": command,
            "changed": false,
            "status": "force_required",
            "exit_code_reason": "force_required",
        }))?;
    } else {
        eprintln!("{command}: refusing to mutate without --force; use --dry-run to preview");
    }
    std::process::exit(2);
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn config_home_for_autostart(ctx: &WallsCtx) -> &std::path::Path {
    ctx.paths
        .config_dir
        .parent()
        .unwrap_or(ctx.paths.config_dir.as_path())
}

fn doctor_options(ctx: &WallsCtx) -> DoctorOptions {
    let tray_runtime = bin_utils::tray_runtime_status();
    DoctorOptions {
        xdg_current_desktop: env_var("XDG_CURRENT_DESKTOP"),
        xdg_session_desktop: env_var("XDG_SESSION_DESKTOP"),
        desktop_startup_id: env_var("DESKTOP_STARTUP_ID"),
        xdg_session_type: env_var("XDG_SESSION_TYPE"),
        wayland_display: env_var("WAYLAND_DISPLAY"),
        display: env_var("DISPLAY"),
        walls_tray: env_var("WALLS_TRAY"),
        walls_tui_preview: env_var("WALLS_TUI_PREVIEW"),
        config_home: Some(config_home_for_autostart(ctx).to_path_buf()),
        tray_bin: Some(tray_runtime.resolved_bin),
        tray_bin_exists: Some(tray_runtime.resolved_bin_exists),
        tray_running: Some(tray_runtime.running),
    }
}

fn desktop_status_json(ctx: &WallsCtx) -> serde_json::Value {
    let xdg_current_desktop = env_var("XDG_CURRENT_DESKTOP");
    let xdg_session_desktop = env_var("XDG_SESSION_DESKTOP");
    let desktop_startup_id = env_var("DESKTOP_STARTUP_ID");
    let xdg_session_type = env_var("XDG_SESSION_TYPE");
    let wayland_display = env_var("WAYLAND_DISPLAY");
    let display = env_var("DISPLAY");
    let walls_tray = env_var("WALLS_TRAY");
    let walls_tray_bin = env_var("WALLS_TRAY_BIN");
    let apply = summarize_apply_environment(&ctx.config.apply);
    let tray_action = walls_core::tray::decide_tray_action();
    let tray_runtime = bin_utils::tray_runtime_status();
    let desktop = walls_core::apply::detect_desktop_from_env(
        xdg_current_desktop.as_deref(),
        xdg_session_desktop.as_deref(),
        desktop_startup_id.as_deref(),
    );
    let autostart_opts = walls_core::autostart::AutostartSyncOpts {
        config_home: config_home_for_autostart(ctx),
        tray_bin: tray_runtime.resolved_bin.clone(),
        config: &ctx.config,
        xdg_current_desktop: xdg_current_desktop.as_deref(),
        xdg_session_desktop: xdg_session_desktop.as_deref(),
        desktop_startup_id: desktop_startup_id.as_deref(),
        xdg_session_type: xdg_session_type.as_deref(),
        wayland_display: wayland_display.as_deref(),
        display: display.as_deref(),
    };
    let tray_action_json = match tray_action {
        walls_core::tray::TrayAction::Spawn => serde_json::json!({
            "action": "spawn",
            "reason": null,
        }),
        walls_core::tray::TrayAction::Skip { reason } => serde_json::json!({
            "action": "skip",
            "reason": reason,
        }),
    };

    serde_json::json!({
        "environment": {
            "XDG_CURRENT_DESKTOP": xdg_current_desktop,
            "XDG_SESSION_DESKTOP": xdg_session_desktop,
            "DESKTOP_STARTUP_ID": desktop_startup_id,
            "XDG_SESSION_TYPE": xdg_session_type,
            "WAYLAND_DISPLAY": wayland_display,
            "DISPLAY": display,
            "WALLS_TRAY": walls_tray,
            "WALLS_TRAY_BIN": walls_tray_bin,
        },
        "detected": {
            "desktop": desktop_display_name(apply.detected_desktop),
            "autostart_desktop": walls_core::tray::desktop_display_name(desktop),
        },
        "apply": {
            "configured_backend": backend_setting_label(apply.configured_backend),
            "resolved_backend": backend_setting_label(apply.resolved_backend),
            "effective_backend": apply.effective_backend_label(),
            "uses_feh_fallback": apply.uses_feh_fallback,
            "cosmic_config_path": apply.cosmic_config_path,
            "cosmic_config_exists": apply.cosmic_config_exists,
        },
        "tray": {
            "launch": tray_action_json,
            "resolved_bin": tray_runtime.resolved_bin.display().to_string(),
            "resolved_bin_exists": tray_runtime.resolved_bin_exists,
            "running": tray_runtime.running,
            "autostart": {
                "desktop": walls_core::tray::desktop_display_name(desktop),
                "available": walls_core::autostart::tray_autostart_available(desktop),
                "desired": walls_core::autostart::tray_autostart_enabled_for_desktop(&ctx.config, desktop),
                "out_of_sync": walls_core::autostart::autostart_out_of_sync(&autostart_opts),
                "desktop_file": walls_core::autostart::autostart_desktop_file_path(config_home_for_autostart(ctx)).display().to_string(),
            },
        },
    })
}

fn print_doctor_human(report: &DoctorReport) {
    println!(
        "walls doctor: {}",
        if report.ready {
            "ready"
        } else {
            "needs attention"
        }
    );
    for section in [
        DoctorSection::Config,
        DoctorSection::DesktopApply,
        DoctorSection::Tray,
        DoctorSection::Providers,
        DoctorSection::StorageCache,
        DoctorSection::Tui,
    ] {
        let checks: Vec<_> = report
            .checks
            .iter()
            .filter(|check| check.section == section)
            .collect();
        if checks.is_empty() {
            continue;
        }
        println!();
        println!("{}", section.title());
        for check in checks {
            let marker = match check.status {
                DoctorStatus::Pass => "ok",
                DoctorStatus::Warn => "warn",
                DoctorStatus::Fail => "fail",
            };
            println!("- [{marker}] {}: {}", check.id, check.message);
            if let Some(remediation) = &check.remediation {
                println!("  fix: {remediation}");
            }
        }
    }
}

fn cmd_doctor(json: bool) -> anyhow::Result<()> {
    let ctx = WallsCtx::load()?;
    let report = walls_core::doctor::run_doctor(&ctx, &doctor_options(&ctx));
    let failed = report.has_failures();
    if json {
        print_json(serde_json::to_value(&report)?)?;
    } else {
        print_doctor_human(&report);
    }
    if failed {
        std::process::exit(1);
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

fn next_result(
    changed: bool,
    status: &str,
    path: Option<PathBuf>,
    exit_code_reason: Option<&str>,
    provider_report: &ProviderStatusReport,
) -> serde_json::Value {
    serde_json::json!({
        "command": "next",
        "changed": changed,
        "status": status,
        "path": path.map(|path| path.display().to_string()),
        "exit_code_reason": exit_code_reason,
        "provider_attempts": &provider_report.attempts,
    })
}

async fn cmd_next(
    manual: bool,
    refresh: Option<CliRefreshLevel>,
    dry_run: bool,
    verbose: bool,
    json: bool,
) -> anyhow::Result<()> {
    if dry_run {
        let ctx = WallsCtx::load_without_autostart_sync()?;
        let plan = next_dry_run_json(&ctx, manual)?;
        return print_json_or_human(json, plan, || {
            print_next_dry_run_human(&ctx, manual)?;
            Ok(())
        });
    }
    let mut ctx = WallsCtx::load()?;
    if let Some(level) = refresh {
        match ctx.refresh_current(level.into())? {
            Some(p) if json => print_json(next_result(
                true,
                "refreshed",
                Some(p),
                None,
                &ctx.provider_status_report,
            ))?,
            Some(p) => {
                println!("{}", p.display());
                print_provider_attempts_human(verbose, &ctx.provider_status_report);
            }
            None if json => print_json(next_result(
                false,
                "missing_current",
                None,
                Some("missing_current"),
                &ctx.provider_status_report,
            ))?,
            None => println!("{}", recovery::missing_current_wallpaper()),
        }
        return Ok(());
    }
    let applied = if manual {
        ctx.advance_next_manual().await?
    } else {
        ctx.advance_next().await?
    };
    match applied {
        Some(p) if json => print_json(next_result(
            true,
            "applied",
            Some(p),
            None,
            &ctx.provider_status_report,
        ))?,
        Some(p) => {
            println!("{}", p.display());
            print_provider_attempts_human(verbose, &ctx.provider_status_report);
        }
        None if json => print_json(next_result(
            false,
            "no_change",
            None,
            Some("no_change"),
            &ctx.provider_status_report,
        ))?,
        None => {
            println!("{}", recovery::next_no_change());
            print_provider_attempts_human(verbose, &ctx.provider_status_report);
        }
    }
    Ok(())
}

fn next_dry_run_json(ctx: &WallsCtx, manual: bool) -> anyhow::Result<serde_json::Value> {
    let skip_reason = next_skip_reason(ctx, manual);
    let local_candidates = ctx.collect_local_candidates()?;
    let provider_plan: Vec<_> =
        walls_core::providers::configured_providers(&ctx.config, &ctx.secrets)
            .into_iter()
            .map(|provider| {
                serde_json::json!({
                    "id": provider.id,
                    "kind": provider.kind,
                    "enabled": provider.enabled,
                })
            })
            .collect();
    let queue_head = ctx.state.cache_queue.first().cloned();
    let would_apply =
        skip_reason.is_none() && (queue_head.is_some() || !local_candidates.is_empty());
    let status = match (skip_reason, would_apply, queue_head.is_some()) {
        (Some(_), _, _) => "would_skip",
        (None, true, true) => "would_try_cached_queue",
        (None, true, false) => "would_try_sources",
        (None, false, _) => "no_candidates",
    };
    let summary = summarize_apply_environment(&ctx.config.apply);
    Ok(serde_json::json!({
        "command": "next",
        "changed": false,
        "status": status,
        "dry_run": true,
        "manual": manual,
        "path": serde_json::Value::Null,
        "exit_code_reason": if status == "no_candidates" {
            serde_json::Value::String("no_change".into())
        } else {
            serde_json::Value::Null
        },
        "next": {
            "skip_reason": skip_reason,
            "cache_queue_len": ctx.state.cache_queue.len(),
            "cache_queue_head": queue_head,
            "local_candidate_count": local_candidates.len(),
            "sample_local_candidate": local_candidates.first(),
            "providers": provider_plan,
            "configured_backend": backend_setting_label(summary.configured_backend),
            "resolved_backend": backend_setting_label(summary.resolved_backend),
            "would_fetch_or_download": false,
            "would_run_backend": would_apply,
            "would_update_current": would_apply,
            "would_update_history": would_apply,
            "would_record_events": would_apply || skip_reason.is_some(),
        },
        "provider_attempts": [],
    }))
}

fn print_next_dry_run_human(ctx: &WallsCtx, manual: bool) -> anyhow::Result<()> {
    if let Some(reason) = next_skip_reason(ctx, manual) {
        println!("would skip next: {reason}");
        println!("no wallpaper files, state, history, or event journal were changed");
        return Ok(());
    }
    let local_candidates = ctx.collect_local_candidates()?;
    println!(
        "would consider cache queue: {} entries",
        ctx.state.cache_queue.len()
    );
    println!(
        "would consider local candidates: {}",
        local_candidates.len()
    );
    if let Some(path) = local_candidates.first() {
        println!("sample local candidate: {}", path.display());
    }
    println!(
        "would run backend if a candidate is selected: {}",
        backend_setting_label(walls_core::apply::resolved_apply_backend(&ctx.config.apply))
    );
    println!("would update current wallpaper state and history if a candidate is selected");
    println!("would record provider/apply events if a candidate is selected");
    println!("no wallpaper files, state, history, or event journal were changed");
    Ok(())
}

fn next_skip_reason(ctx: &WallsCtx, manual: bool) -> Option<&'static str> {
    if manual {
        return None;
    }
    if ctx.state.paused {
        return Some("paused");
    }
    if !ctx.config.change.enabled {
        return Some("change_disabled");
    }
    None
}

fn print_provider_attempts_human(verbose: bool, report: &ProviderStatusReport) {
    if !verbose {
        return;
    }
    if report.attempts.is_empty() {
        println!("provider attempts: none recorded");
        return;
    }
    println!("provider attempts:");
    for attempt in &report.attempts {
        println!("  - {}", provider_attempt_line(attempt));
        for retry in &attempt.retries {
            let status = retry
                .status_code
                .map(|code| format!(" status {code}"))
                .unwrap_or_default();
            println!(
                "    retry {}: {}{} after {}ms",
                retry.attempt,
                retry_reason_label(retry.reason),
                status,
                retry.backoff_ms
            );
        }
        if let Some(fallback) = &attempt.fallback_provider_id {
            println!("    fallback: {fallback}");
        }
    }
}

fn provider_attempt_line(attempt: &ProviderAttempt) -> String {
    let prefix = format!(
        "{} ({}) {} [{}]",
        attempt.provider_id,
        provider_kind_label(attempt.provider_kind),
        provider_operation_label(attempt.operation),
        provider_status_label(attempt.status)
    );
    match &attempt.outcome {
        ProviderAttemptOutcome::NotRun => format!("{prefix}: not run"),
        ProviderAttemptOutcome::Applied { candidate_count } => {
            format!(
                "{prefix}: applied{}",
                candidate_count_suffix(*candidate_count)
            )
        }
        ProviderAttemptOutcome::Skipped { reason } => {
            format!(
                "{prefix}: skipped ({}); {}",
                no_candidate_reason_label(*reason),
                no_candidate_recovery_hint(*reason)
            )
        }
        ProviderAttemptOutcome::NoCandidates {
            reason,
            candidate_count,
        } => format!(
            "{prefix}: no candidates ({}){}; {}",
            no_candidate_reason_label(*reason),
            candidate_count_suffix(*candidate_count),
            no_candidate_recovery_hint(*reason)
        ),
        ProviderAttemptOutcome::Failed {
            kind,
            status_code,
            message,
        } => {
            let status = status_code
                .map(|code| format!(" status {code}"))
                .unwrap_or_default();
            let message = message
                .as_deref()
                .map(|message| format!(": {message}"))
                .unwrap_or_default();
            format!(
                "{prefix}: failed ({}{}){}",
                failure_kind_label(*kind),
                status,
                message
            )
        }
    }
}

fn candidate_count_suffix(candidate_count: Option<usize>) -> String {
    candidate_count
        .map(|count| format!(" ({count} candidate{})", if count == 1 { "" } else { "s" }))
        .unwrap_or_default()
}

fn provider_kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Local => "local",
        ProviderKind::Wallhaven => "wallhaven",
        ProviderKind::Unsplash => "unsplash",
        ProviderKind::Reddit => "reddit",
        ProviderKind::Bing => "bing",
        ProviderKind::Apod => "apod",
        ProviderKind::MediaRss => "mediarss",
        ProviderKind::Attribution => "attribution",
        ProviderKind::Json => "json",
        ProviderKind::Pixabay => "pixabay",
        ProviderKind::Immich => "immich",
        ProviderKind::Spotlight => "spotlight",
        ProviderKind::Weighting => "weighting",
        ProviderKind::Unsupported => "unsupported",
    }
}

fn provider_operation_label(operation: ProviderOperation) -> &'static str {
    match operation {
        ProviderOperation::AdvanceNext => "advance_next",
        ProviderOperation::QueueRefill => "queue_refill",
        ProviderOperation::Search => "search",
        ProviderOperation::Download => "download",
        ProviderOperation::Metadata => "metadata",
        ProviderOperation::DoctorCheck => "doctor_check",
        ProviderOperation::LocalSourceListing => "local_source_listing",
    }
}

fn provider_status_label(status: ProviderStatus) -> &'static str {
    match status {
        ProviderStatus::Enabled => "enabled",
        ProviderStatus::Disabled => "disabled",
        ProviderStatus::OfflineDisabled => "offline-disabled",
        ProviderStatus::CredentialMissing => "credential-missing",
    }
}

fn no_candidate_reason_label(reason: ProviderNoCandidateReason) -> &'static str {
    match reason {
        ProviderNoCandidateReason::Disabled => "disabled",
        ProviderNoCandidateReason::OfflineDisabled => "offline disabled",
        ProviderNoCandidateReason::CredentialMissing => "credential missing",
        ProviderNoCandidateReason::QueueEmpty => "queue empty",
        ProviderNoCandidateReason::NoEnabledSource => "no enabled source",
        ProviderNoCandidateReason::EmptyResult => "empty result",
        ProviderNoCandidateReason::FilteredByHistory => "filtered by history",
        ProviderNoCandidateReason::Unsupported => "unsupported",
    }
}

fn no_candidate_recovery_hint(reason: ProviderNoCandidateReason) -> &'static str {
    match reason {
        ProviderNoCandidateReason::Disabled => {
            "enable the source in config or choose another enabled provider"
        }
        ProviderNoCandidateReason::OfflineDisabled => {
            "run `walls next --manual --verbose` when online sources are enabled, or keep a local fallback source enabled"
        }
        ProviderNoCandidateReason::CredentialMissing => {
            "add the provider credential to secrets.json or disable this source"
        }
        ProviderNoCandidateReason::QueueEmpty => {
            "run `walls cache status` and enable/fetch provider candidates before retrying"
        }
        ProviderNoCandidateReason::NoEnabledSource => {
            "enable at least one source with candidates in config.json"
        }
        ProviderNoCandidateReason::EmptyResult => {
            "loosen provider filters or search terms, then retry with `walls next --manual --verbose`"
        }
        ProviderNoCandidateReason::FilteredByHistory => {
            "add more wallpapers or reduce selection.avoid_recent"
        }
        ProviderNoCandidateReason::Unsupported => {
            "check the source type and run `walls config validate`"
        }
    }
}

fn failure_kind_label(kind: ProviderFailureKind) -> &'static str {
    match kind {
        ProviderFailureKind::Request => "request",
        ProviderFailureKind::RateLimited => "rate limited",
        ProviderFailureKind::Timeout => "timeout",
        ProviderFailureKind::Connect => "connect",
        ProviderFailureKind::Decode => "decode",
        ProviderFailureKind::Io => "io",
        ProviderFailureKind::Config => "config",
        ProviderFailureKind::Unknown => "unknown",
    }
}

fn retry_reason_label(reason: ProviderRetryReason) -> &'static str {
    match reason {
        ProviderRetryReason::RateLimited => "rate limited",
        ProviderRetryReason::ServerError => "server error",
        ProviderRetryReason::Timeout => "timeout",
        ProviderRetryReason::Connect => "connect",
    }
}

fn cmd_prev(json: bool) -> anyhow::Result<()> {
    cmd_restore_previous("prev", "applied_previous", json)
}

fn cmd_undo(json: bool) -> anyhow::Result<()> {
    cmd_restore_previous("undo", "restored_previous", json)
}

fn cmd_restore_previous(command: &str, status: &str, json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_prev() {
        Ok(Some(p)) if json => print_json(command_result(command, true, status, Some(p), None))?,
        Ok(Some(p)) => println!("{}", p.display()),
        Ok(None) if json => print_json(command_result(
            command,
            false,
            "no_previous",
            None,
            Some("no_previous"),
        ))?,
        Ok(None) => println!("{}", recovery::no_previous_wallpaper()),
        Err(WallsError::PreviousOriginalMissing { path }) if json => {
            print_json(command_result(
                command,
                false,
                "missing_previous",
                Some(path),
                Some("missing_previous"),
            ))?;
            std::process::exit(1);
        }
        Err(WallsError::PreviousOriginalMissing { path }) => {
            eprintln!("{}", recovery::missing_previous_wallpaper(&path));
            std::process::exit(1);
        }
        Err(error) => return Err(error.into()),
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

fn cmd_config_sync(dry_run: bool) -> anyhow::Result<()> {
    let ctx = if dry_run {
        WallsCtx::load_without_autostart_sync()?
    } else {
        WallsCtx::load()?
    };
    let outcome = if dry_run {
        walls_core::autostart::dry_run_tray_autostart_sync(&ctx.config)?
    } else {
        walls_core::autostart::sync_tray_autostart(&ctx.config)?
    };
    match outcome {
        walls_core::autostart::AutostartSyncOutcome::Written => {
            println!("tray autostart: updated");
        }
        walls_core::autostart::AutostartSyncOutcome::Removed => {
            println!("tray autostart: removed");
        }
        walls_core::autostart::AutostartSyncOutcome::WouldWrite { path } => {
            println!("tray autostart: would update {}", path.display());
        }
        walls_core::autostart::AutostartSyncOutcome::WouldRemove { path } => {
            println!("tray autostart: would remove {}", path.display());
        }
        walls_core::autostart::AutostartSyncOutcome::Skipped { reason } => {
            println!("tray autostart: skipped ({reason})");
        }
    }
    Ok(())
}

fn cmd_trash(dry_run: bool, force: bool, json: bool) -> anyhow::Result<()> {
    let mut ctx = WallsCtx::load()?;
    let plan = ctx.plan_trash_current()?;
    if dry_run {
        return print_json_or_human(json, trash_plan_json(&plan, true), || {
            println!("would trash original: {}", plan.original_path);
            if let Some(composed) = &plan.composed_path {
                println!("would trash composed: {composed}");
            }
            println!(
                "would remove history entries: {}",
                plan.history_entries_removed
            );
            if let Some(id) = &plan.cache_queue_id {
                println!("would remove cache queue id: {id}");
            }
            Ok(())
        });
    }
    require_force(force, json, "trash")?;
    ctx.trash_current()?;
    print_json_or_human(
        json,
        serde_json::json!({
            "command": "trash",
            "changed": true,
            "status": "trashed",
            "dry_run": false,
            "trash": trash_plan_details_json(&plan),
            "exit_code_reason": null,
        }),
        || {
            println!("trashed");
            Ok(())
        },
    )
}

fn trash_plan_json(plan: &walls_core::ctx::TrashPlan, dry_run: bool) -> serde_json::Value {
    serde_json::json!({
        "command": "trash",
        "changed": false,
        "status": if dry_run { "would_trash" } else { "trash_plan" },
        "dry_run": dry_run,
        "trash": trash_plan_details_json(plan),
        "exit_code_reason": null,
    })
}

fn trash_plan_details_json(plan: &walls_core::ctx::TrashPlan) -> serde_json::Value {
    serde_json::json!({
        "original_path": plan.original_path,
        "composed_path": plan.composed_path,
        "original_exists": plan.original_exists,
        "composed_exists": plan.composed_exists,
        "cache_queue_id": plan.cache_queue_id,
        "history_entries_removed": plan.history_entries_removed,
    })
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
            println!("{}", recovery::missing_current_wallpaper());
            std::process::exit(1);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use walls_core::providers::{ProviderCapability, ProviderDescriptor};

    fn provider_attempt(kind: ProviderKind) -> ProviderAttempt {
        ProviderDescriptor {
            id: provider_kind_label(kind).into(),
            kind,
            enabled: true,
            capabilities: vec![ProviderCapability::QueueRefill],
        }
        .attempt(ProviderOperation::AdvanceNext)
    }

    #[test]
    fn provider_attempt_lines_include_recovery_for_missing_credentials() {
        let line = provider_attempt(ProviderKind::Unsplash)
            .with_status(ProviderStatus::CredentialMissing)
            .skipped(ProviderNoCandidateReason::CredentialMissing);

        let line = provider_attempt_line(&line);

        assert!(line.contains("credential missing"), "{line}");
        assert!(line.contains("secrets.json"), "{line}");
    }

    #[test]
    fn provider_attempt_lines_include_recovery_for_empty_and_filtered_results() {
        let empty = provider_attempt(ProviderKind::Wallhaven)
            .no_candidates(ProviderNoCandidateReason::EmptyResult, Some(0));
        let filtered = provider_attempt(ProviderKind::Local)
            .no_candidates(ProviderNoCandidateReason::FilteredByHistory, Some(5));

        let empty = provider_attempt_line(&empty);
        let filtered = provider_attempt_line(&filtered);

        assert!(empty.contains("loosen provider filters"), "{empty}");
        assert!(filtered.contains("selection.avoid_recent"), "{filtered}");
    }
}

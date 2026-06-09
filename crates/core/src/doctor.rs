use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::apply::{
    backend_setting_label, desktop_display_name, summarize_apply_environment_from_env,
};
use crate::autostart::{
    autostart_desktop_file_path, autostart_out_of_sync, tray_autostart_available,
    tray_autostart_enabled_for_desktop, AutostartSyncOpts,
};
use crate::config::{
    secrets_credential_field, secrets_credential_present, source_secrets_key, ApplyBackendSetting,
    SourceKind,
};
use crate::ctx::WallsCtx;
use crate::events::{last_run_summary, read_events, LastRunStatus, LastRunSummary};
use crate::providers::{
    configured_providers, provider_for_source, ProviderAttempt, ProviderNoCandidateReason,
    ProviderOperation, ProviderStatus,
};
use crate::sources::list_images_with_paths;
use crate::tray::{decide_tray_action_from_env, TrayAction};
use crate::validate::{secrets_file_permission_warnings, validate_config_diagnostics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorSection {
    Config,
    DesktopApply,
    Tray,
    Providers,
    StorageCache,
    Tui,
}

impl DoctorSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::Config => "Config",
            Self::DesktopApply => "Desktop/apply",
            Self::Tray => "Tray",
            Self::Providers => "Providers",
            Self::StorageCache => "Storage/cache",
            Self::Tui => "TUI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub id: String,
    pub section: DoctorSection,
    pub severity: DoctorSeverity,
    pub status: DoctorStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl DoctorCheck {
    fn pass(section: DoctorSection, id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            section,
            severity: DoctorSeverity::Info,
            status: DoctorStatus::Pass,
            message: message.into(),
            remediation: None,
        }
    }

    fn warn(
        section: DoctorSection,
        id: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            section,
            severity: DoctorSeverity::Warning,
            status: DoctorStatus::Warn,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }

    fn fail(
        section: DoctorSection,
        id: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            section,
            severity: DoctorSeverity::Error,
            status: DoctorStatus::Fail,
            message: message.into(),
            remediation: Some(remediation.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub ready: bool,
    pub checks: Vec<DoctorCheck>,
    pub provider_attempts: Vec<ProviderAttempt>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == DoctorStatus::Fail)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    pub xdg_current_desktop: Option<String>,
    pub xdg_session_desktop: Option<String>,
    pub desktop_startup_id: Option<String>,
    pub xdg_session_type: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub walls_tray: Option<String>,
    pub walls_tui_preview: Option<String>,
    pub config_home: Option<PathBuf>,
    pub tray_bin: Option<PathBuf>,
    pub tray_bin_exists: Option<bool>,
    pub tray_running: Option<bool>,
}

pub fn run_doctor(ctx: &WallsCtx, options: &DoctorOptions) -> DoctorReport {
    let mut checks = Vec::new();
    check_config(ctx, &mut checks);
    check_desktop_apply(ctx, options, &mut checks);
    check_tray(ctx, options, &mut checks);
    check_providers(ctx, &mut checks);
    check_last_run(ctx, &mut checks);
    check_storage_cache(ctx, &mut checks);
    check_tui(options, &mut checks);
    let provider_attempts = provider_doctor_attempts(ctx);
    DoctorReport {
        ready: !checks
            .iter()
            .any(|check| check.status == DoctorStatus::Fail),
        checks,
        provider_attempts,
    }
}

fn check_last_run(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) {
    let summary = read_events(&ctx.paths.event_journal_file)
        .ok()
        .and_then(|events| last_run_summary(&events));
    match summary.as_ref() {
        Some(summary) => push_last_run_checks(summary, checks),
        None => checks.push(DoctorCheck::pass(
            DoctorSection::Providers,
            "providers.last_run",
            "no recent wallpaper run recorded",
        )),
    }
}

fn push_last_run_checks(summary: &LastRunSummary, checks: &mut Vec<DoctorCheck>) {
    match summary.status {
        LastRunStatus::Applied => {
            checks.push(DoctorCheck::pass(
                DoctorSection::Providers,
                "providers.last_run",
                format!("last run succeeded: {}", summary.message),
            ));
            for warning in &summary.warnings {
                checks.push(DoctorCheck::warn(
                    DoctorSection::Providers,
                    "providers.last_run.warning",
                    format!("last run warning: {warning}"),
                    "run `walls logs --tail 20` to inspect provider skips and fallbacks",
                ));
            }
        }
        LastRunStatus::NoChange => checks.push(DoctorCheck::warn(
            DoctorSection::Providers,
            "providers.last_run.no_change",
            format!("last run made no change: {}", summary.message),
            "run `walls next --manual --verbose` to see provider skips, or check source readiness",
        )),
        LastRunStatus::Failed => {
            checks.push(DoctorCheck::fail(
                DoctorSection::Providers,
                "providers.last_run.failed",
                format!("last run failed: {}", summary.message),
                "run `walls logs --level error --tail 20` for the recent failure details",
            ));
            for error in &summary.errors {
                checks.push(DoctorCheck::fail(
                    DoctorSection::Providers,
                    "providers.last_run.error",
                    format!("last run error: {error}"),
                    "fix the provider or backend error, then run `walls next --manual` again",
                ));
            }
        }
    }
}

fn provider_doctor_attempts(ctx: &WallsCtx) -> Vec<ProviderAttempt> {
    let mut attempts = Vec::new();
    for source in &ctx.config.sources {
        let provider = provider_for_source(source);
        let source_kind = SourceKind::parse(&source.source_type);
        let mut attempt = provider.attempt(ProviderOperation::DoctorCheck);
        if !source.enabled {
            attempt = attempt
                .with_status(ProviderStatus::Disabled)
                .skipped(ProviderNoCandidateReason::Disabled);
        } else if !source_kind.is_local() && !ctx.config.change.internet_enabled {
            attempt = attempt
                .with_status(ProviderStatus::OfflineDisabled)
                .skipped(ProviderNoCandidateReason::OfflineDisabled);
        } else if let Some(key) = source_secrets_key(&source.source_type) {
            if !secrets_credential_present(key, &ctx.secrets) {
                attempt = attempt
                    .with_status(ProviderStatus::CredentialMissing)
                    .skipped(ProviderNoCandidateReason::CredentialMissing);
            }
        } else if source_kind == SourceKind::Unknown {
            attempt = attempt.no_candidates(ProviderNoCandidateReason::Unsupported, None);
        }
        attempts.push(attempt);
    }

    let configured = configured_providers(&ctx.config, &ctx.secrets);
    if let Some(wallhaven) = configured
        .iter()
        .find(|provider| provider.kind == crate::providers::ProviderKind::Wallhaven)
    {
        let mut attempt = wallhaven.attempt(ProviderOperation::DoctorCheck);
        if !ctx.config.wallhaven.enabled {
            attempt = attempt
                .with_status(ProviderStatus::Disabled)
                .skipped(ProviderNoCandidateReason::Disabled);
        } else if !ctx.config.change.internet_enabled {
            attempt = attempt
                .with_status(ProviderStatus::OfflineDisabled)
                .skipped(ProviderNoCandidateReason::OfflineDisabled);
        }
        attempts.push(attempt);
    }
    attempts
}

fn check_config(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) {
    check_required_file(
        checks,
        DoctorSection::Config,
        "config.config_dir",
        &ctx.paths.config_dir,
        "config directory",
        "run `walls` once to create the config directory, or fix XDG_CONFIG_HOME",
    );

    if ctx.paths.config_file.is_file() {
        checks.push(DoctorCheck::pass(
            DoctorSection::Config,
            "config.file",
            format!("config file found at {}", ctx.paths.config_file.display()),
        ));
    } else {
        checks.push(DoctorCheck::fail(
            DoctorSection::Config,
            "config.file",
            format!("config file missing at {}", ctx.paths.config_file.display()),
            "run `walls` once to create a default config",
        ));
    }

    if ctx.paths.secrets_file.exists() {
        checks.push(DoctorCheck::pass(
            DoctorSection::Config,
            "config.secrets_file",
            format!("secrets file found at {}", ctx.paths.secrets_file.display()),
        ));
    } else {
        checks.push(DoctorCheck::warn(
            DoctorSection::Config,
            "config.secrets_file",
            format!(
                "secrets file not found at {}",
                ctx.paths.secrets_file.display()
            ),
            "create secrets.json when enabling providers that need credentials",
        ));
    }

    check_parent_writable(
        checks,
        DoctorSection::Config,
        "config.state_file",
        &ctx.paths.state_file,
        "state file",
        "create the state directory and ensure the current user can write to it",
    );

    let validation = validate_config_diagnostics(&ctx.config, &ctx.secrets, &ctx.paths);
    if validation.is_empty() {
        checks.push(DoctorCheck::pass(
            DoctorSection::Config,
            "config.validation",
            "config validation passed",
        ));
    } else {
        checks.extend(validation.into_iter().map(|diagnostic| {
            DoctorCheck::fail(
                DoctorSection::Config,
                format!("config.validation.{}", diagnostic.path),
                diagnostic.message,
                diagnostic
                    .hint
                    .unwrap_or_else(|| "run `walls config validate` for details".to_string()),
            )
        }));
    }

    checks.extend(
        secrets_file_permission_warnings(&ctx.paths)
            .into_iter()
            .map(|warning| {
                DoctorCheck::warn(
                    DoctorSection::Config,
                    "config.secrets_permissions",
                    warning,
                    "restrict secrets.json permissions with `chmod 600`",
                )
            }),
    );
}

fn check_desktop_apply(ctx: &WallsCtx, options: &DoctorOptions, checks: &mut Vec<DoctorCheck>) {
    let summary = summarize_apply_environment_from_env(
        &ctx.config.apply,
        options.xdg_current_desktop.clone(),
        options.xdg_session_desktop.clone(),
        options.xdg_session_type.clone(),
    );
    checks.push(DoctorCheck::pass(
        DoctorSection::DesktopApply,
        "desktop.detected",
        format!(
            "desktop detected as {}",
            desktop_display_name(summary.detected_desktop)
        ),
    ));
    if summary.uses_feh_fallback {
        checks.push(DoctorCheck::warn(
            DoctorSection::DesktopApply,
            "desktop.apply_backend",
            "auto backend resolved to feh/nitrogen fallback",
            "set apply.backend explicitly or install/configure feh or nitrogen",
        ));
    } else {
        checks.push(DoctorCheck::pass(
            DoctorSection::DesktopApply,
            "desktop.apply_backend",
            format!(
                "apply backend resolved to {}",
                backend_setting_label(summary.resolved_backend)
            ),
        ));
    }

    check_apply_backend_commands(ctx, summary.resolved_backend, checks);

    if summary.cosmic_config_exists == Some(false) {
        checks.push(DoctorCheck::fail(
            DoctorSection::DesktopApply,
            "desktop.cosmic_config",
            format!(
                "COSMIC config path is missing: {}",
                summary.cosmic_config_path.unwrap_or_default()
            ),
            "correct apply.cosmic.config_path or switch apply.backend",
        ));
    }
}

fn check_apply_backend_commands(
    ctx: &WallsCtx,
    backend: ApplyBackendSetting,
    checks: &mut Vec<DoctorCheck>,
) {
    match backend {
        ApplyBackendSetting::Auto => {
            check_any_command_warn(
                checks,
                DoctorSection::DesktopApply,
                "desktop.apply_command",
                &["feh", "nitrogen"],
                "feh/nitrogen fallback command is available",
                "install feh or nitrogen, or set apply.backend for your desktop",
            );
        }
        ApplyBackendSetting::CustomScript => {
            let Some(script) = &ctx.config.apply.custom_script else {
                return;
            };
            let script = crate::paths::expand_home(script);
            if script.is_file() {
                checks.push(DoctorCheck::pass(
                    DoctorSection::DesktopApply,
                    "desktop.apply_command",
                    format!("custom apply script found at {}", script.display()),
                ));
            }
        }
        ApplyBackendSetting::Gnome => check_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            "gsettings",
            "gsettings is available for GNOME wallpaper apply",
            "install gsettings, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Kde => check_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            "dbus-send",
            "dbus-send is available for KDE wallpaper apply",
            "install dbus-send, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Xfce => check_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            "xfconf-query",
            "xfconf-query is available for XFCE wallpaper apply",
            "install xfconf-query, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Sway => check_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            "swaymsg",
            "swaymsg is available for Sway wallpaper apply",
            "install swaymsg, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Wlroots => check_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            "swaybg",
            "swaybg is available for wlroots wallpaper apply",
            "install swaybg, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Hyprland => check_all_commands(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            &["swaybg", "hyprctl"],
            "Hyprland apply helper commands are available",
            "install swaybg and hyprctl, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Feh => check_any_command(
            checks,
            DoctorSection::DesktopApply,
            "desktop.apply_command",
            &["feh", "nitrogen"],
            "feh or nitrogen is available for wallpaper apply",
            "install feh or nitrogen, or choose a different apply.backend",
        ),
        ApplyBackendSetting::Cosmic | ApplyBackendSetting::CosmicExtBgCtl => {
            if ctx.config.apply.cosmic.method == crate::config::CosmicMethod::CosmicExtBgCtl
                || backend == ApplyBackendSetting::CosmicExtBgCtl
            {
                check_command(
                    checks,
                    DoctorSection::DesktopApply,
                    "desktop.apply_command",
                    "cosmic-ext-bg-ctl",
                    "cosmic-ext-bg-ctl is available for COSMIC wallpaper apply",
                    "install cosmic-ext-bg-ctl, switch COSMIC method, or choose a different apply.backend",
                );
            }
        }
    }
}

fn check_tray(ctx: &WallsCtx, options: &DoctorOptions, checks: &mut Vec<DoctorCheck>) {
    let action = decide_tray_action_from_env(
        options.walls_tray.as_deref(),
        options.xdg_current_desktop.as_deref(),
        options.xdg_session_desktop.as_deref(),
        options.desktop_startup_id.as_deref(),
        options.xdg_session_type.as_deref(),
        options.wayland_display.as_deref(),
        options.display.as_deref(),
    );
    match action {
        TrayAction::Spawn => checks.push(DoctorCheck::pass(
            DoctorSection::Tray,
            "tray.launch",
            "tray launch is available in this session",
        )),
        TrayAction::Skip { reason } => checks.push(DoctorCheck::warn(
            DoctorSection::Tray,
            "tray.launch",
            reason,
            "set WALLS_TRAY=1 to force tray launch or use `walls tui` without tray",
        )),
    }

    if let Some(running) = options.tray_running {
        checks.push(if running {
            DoctorCheck::pass(DoctorSection::Tray, "tray.running", "walls-tray is running")
        } else {
            DoctorCheck::warn(
                DoctorSection::Tray,
                "tray.running",
                "walls-tray is not running",
                "start `walls-tray` or launch `walls tui` in a supported desktop session",
            )
        });
    }

    let desktop = crate::apply::detect_desktop_from_env(
        options.xdg_current_desktop.as_deref(),
        options.xdg_session_desktop.as_deref(),
        options.desktop_startup_id.as_deref(),
    );
    if tray_autostart_available(desktop) {
        checks.push(DoctorCheck::pass(
            DoctorSection::Tray,
            "tray.autostart_available",
            "tray autostart is available for this desktop",
        ));
    } else {
        checks.push(DoctorCheck::warn(
            DoctorSection::Tray,
            "tray.autostart_available",
            "tray autostart is not available for this desktop",
            "use manual tray launch or the TUI controls",
        ));
    }

    if let (Some(config_home), Some(tray_bin)) = (&options.config_home, &options.tray_bin) {
        let opts = AutostartSyncOpts {
            config_home,
            tray_bin: tray_bin.clone(),
            config: &ctx.config,
            xdg_current_desktop: options.xdg_current_desktop.as_deref(),
            xdg_session_desktop: options.xdg_session_desktop.as_deref(),
            desktop_startup_id: options.desktop_startup_id.as_deref(),
            xdg_session_type: options.xdg_session_type.as_deref(),
            wayland_display: options.wayland_display.as_deref(),
            display: options.display.as_deref(),
        };
        let enabled = tray_autostart_enabled_for_desktop(&ctx.config, desktop);
        let path = autostart_desktop_file_path(config_home);
        if enabled && autostart_out_of_sync(&opts) {
            checks.push(DoctorCheck::warn(
                DoctorSection::Tray,
                "tray.autostart_sync",
                format!("tray autostart is out of sync at {}", path.display()),
                "run `walls config sync`",
            ));
        } else {
            checks.push(DoctorCheck::pass(
                DoctorSection::Tray,
                "tray.autostart_sync",
                "tray autostart config is in sync",
            ));
        }
    }

    if let (Some(tray_bin), Some(exists)) = (&options.tray_bin, options.tray_bin_exists) {
        checks.push(if exists {
            DoctorCheck::pass(
                DoctorSection::Tray,
                "tray.binary",
                format!("walls-tray binary found at {}", tray_bin.display()),
            )
        } else {
            DoctorCheck::warn(
                DoctorSection::Tray,
                "tray.binary",
                format!("walls-tray binary not found at {}", tray_bin.display()),
                "install walls-tray or set WALLS_TRAY_BIN",
            )
        });
    }
}

fn check_providers(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) {
    let enabled_sources = ctx
        .config
        .sources
        .iter()
        .filter(|source| source.enabled)
        .count();
    if enabled_sources == 0 {
        checks.push(DoctorCheck::fail(
            DoctorSection::Providers,
            "providers.enabled_sources",
            "no enabled wallpaper sources are configured",
            "enable at least one source in config.json",
        ));
        return;
    }
    checks.push(DoctorCheck::pass(
        DoctorSection::Providers,
        "providers.enabled_sources",
        format!("{enabled_sources} enabled source(s) configured"),
    ));

    let local_sources = ctx
        .config
        .sources
        .iter()
        .filter(|source| source.enabled && SourceKind::parse(&source.source_type).is_local())
        .count();
    if local_sources > 0 {
        checks.push(DoctorCheck::pass(
            DoctorSection::Providers,
            "providers.local_sources",
            format!("{local_sources} enabled local source(s) configured"),
        ));
    } else if !ctx.config.change.internet_enabled {
        checks.push(DoctorCheck::fail(
            DoctorSection::Providers,
            "providers.local_sources",
            "internet is disabled and no enabled local sources are configured",
            "enable a folder/favorites/fetched source or turn change.internet_enabled on",
        ));
    }

    for (index, source) in ctx.config.sources.iter().enumerate() {
        if !source.enabled {
            continue;
        }
        let source_kind = SourceKind::parse(&source.source_type);
        if source_kind.is_local() {
            check_local_source_candidates(ctx, index, checks);
        } else if ctx.config.change.internet_enabled {
            checks.push(DoctorCheck::warn(
                DoctorSection::Providers,
                format!("providers.source_{index}.candidates"),
                format!(
                    "source {} is online-backed and was not live-checked",
                    source.source_type
                ),
                "run `walls next --json` to inspect live provider attempts, or add a local fallback source",
            ));
        }
        if let Some(key) = source_secrets_key(&source.source_type) {
            let present = secrets_credential_present(key, &ctx.secrets);
            checks.push(if present || !ctx.config.change.internet_enabled {
                DoctorCheck::pass(
                    DoctorSection::Providers,
                    format!("providers.source_{index}.credentials"),
                    format!("credentials ready for source {}", source.source_type),
                )
            } else {
                DoctorCheck::fail(
                    DoctorSection::Providers,
                    format!("providers.source_{index}.credentials"),
                    format!("missing credentials for source {}", source.source_type),
                    source_credentials_remediation(key),
                )
            });
        }
    }

    let verified_local_candidates = verified_local_candidate_count(ctx, checks);
    if verified_local_candidates > 0 {
        checks.push(DoctorCheck::pass(
            DoctorSection::Providers,
            "providers.candidate_readiness",
            format!("{verified_local_candidates} verified local candidate(s) available"),
        ));
    } else if ctx.config.change.internet_enabled && has_online_provider(ctx) {
        checks.push(DoctorCheck::warn(
            DoctorSection::Providers,
            "providers.candidate_readiness",
            "no local candidates were verified; readiness depends on online providers",
            "run `walls next --json` to verify live provider attempts or add a local fallback source",
        ));
    } else {
        checks.push(DoctorCheck::fail(
            DoctorSection::Providers,
            "providers.candidate_readiness",
            "no configured source can currently produce a verified wallpaper candidate",
            "add images to a local source, enable a provider, or turn internet providers on",
        ));
    }
}

fn source_credentials_remediation(key: crate::config::SourceSecretsKey) -> String {
    format!(
        "add `{}` to secrets.json, run `walls config validate`, or disable this source",
        secrets_credential_field(key)
    )
}

fn check_local_source_candidates(ctx: &WallsCtx, index: usize, checks: &mut Vec<DoctorCheck>) {
    let Some(source) = ctx.config.sources.get(index) else {
        return;
    };
    match list_images_with_paths(source, &ctx.paths.favorites_dir, &ctx.paths.fetched_dir) {
        Ok(images) if images.is_empty() => checks.push(DoctorCheck::warn(
            DoctorSection::Providers,
            format!("providers.source_{index}.candidates"),
            format!(
                "local source {} has no image candidates",
                local_source_label(source)
            ),
            "add jpg, jpeg, png, webp, avif, bmp, or gif files to this source",
        )),
        Ok(images) => checks.push(DoctorCheck::pass(
            DoctorSection::Providers,
            format!("providers.source_{index}.candidates"),
            format!(
                "local source {} has {} image candidate(s)",
                local_source_label(source),
                images.len()
            ),
        )),
        Err(error) => checks.push(DoctorCheck::fail(
            DoctorSection::Providers,
            format!("providers.source_{index}.candidates"),
            format!(
                "could not inspect local source {}: {error:#}",
                local_source_label(source)
            ),
            "fix the source path or disable this source",
        )),
    }
}

fn verified_local_candidate_count(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) -> usize {
    match ctx.collect_local_candidates() {
        Ok(candidates) => candidates.len(),
        Err(error) => {
            checks.push(DoctorCheck::fail(
                DoctorSection::Providers,
                "providers.local_candidate_listing",
                format!("could not inspect local candidates: {error:#}"),
                "fix local source paths or disable broken local sources",
            ));
            0
        }
    }
}

fn has_online_provider(ctx: &WallsCtx) -> bool {
    ctx.config.wallhaven.enabled
        || ctx
            .config
            .sources
            .iter()
            .any(|source| source.enabled && !SourceKind::parse(&source.source_type).is_local())
}

fn local_source_label(source: &crate::config::SourceEntry) -> String {
    source
        .label
        .clone()
        .unwrap_or_else(|| source.source_type.clone())
}

fn check_storage_cache(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) {
    for (id, path) in [
        ("storage.cache_dir", &ctx.paths.cache_dir),
        ("storage.download_dir", &ctx.paths.download_dir),
        ("storage.favorites_dir", &ctx.paths.favorites_dir),
        ("storage.fetched_dir", &ctx.paths.fetched_dir),
        ("storage.compose_dir", &ctx.paths.compose_dir),
    ] {
        if dir_is_writable(path) {
            checks.push(DoctorCheck::pass(
                DoctorSection::StorageCache,
                id,
                format!("{} is writable", path.display()),
            ));
        } else {
            checks.push(DoctorCheck::fail(
                DoctorSection::StorageCache,
                id,
                format!("{} is not writable", path.display()),
                "create the directory and ensure the current user can write to it",
            ));
        }
    }

    let inspection = ctx.inspect_cache();
    checks.push(DoctorCheck::pass(
        DoctorSection::StorageCache,
        "storage.download_usage",
        format!(
            "download storage has {} file(s), {} bytes used",
            inspection.downloads.files, inspection.downloads.bytes
        ),
    ));

    if ctx.config.quota.enabled && ctx.config.quota.size_mb == 0 {
        checks.push(DoctorCheck::warn(
            DoctorSection::StorageCache,
            "storage.quota",
            "download quota is enabled with a zero MiB limit",
            "set quota.size_mb above zero or disable quota",
        ));
    } else if ctx.config.quota.enabled {
        let quota_bytes = ctx.config.quota.size_mb.saturating_mul(1024 * 1024);
        if inspection.downloads.bytes > quota_bytes {
            checks.push(DoctorCheck::warn(
                DoctorSection::StorageCache,
                "storage.quota",
                format!(
                    "download storage is {} bytes over the configured quota",
                    inspection.downloads.bytes - quota_bytes
                ),
                "run `walls cache status` and `walls cache prune --dry-run` before pruning with --force",
            ));
        } else {
            checks.push(DoctorCheck::pass(
                DoctorSection::StorageCache,
                "storage.quota",
                format!(
                    "download quota is enabled with {} bytes remaining",
                    quota_bytes - inspection.downloads.bytes
                ),
            ));
        }
    } else {
        checks.push(DoctorCheck::pass(
            DoctorSection::StorageCache,
            "storage.quota",
            format!(
                "download quota is {}",
                if ctx.config.quota.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        ));
    }
}

fn check_required_file(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    path: &Path,
    label: &'static str,
    remediation: &'static str,
) {
    if path.is_dir() {
        checks.push(DoctorCheck::pass(
            section,
            id,
            format!("{label} found at {}", path.display()),
        ));
    } else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!("{label} missing at {}", path.display()),
            remediation,
        ));
    }
}

fn check_parent_writable(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    path: &Path,
    label: &'static str,
    remediation: &'static str,
) {
    let Some(parent) = path.parent() else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!("{label} has no parent directory: {}", path.display()),
            remediation,
        ));
        return;
    };
    if dir_is_writable(parent) {
        checks.push(DoctorCheck::pass(
            section,
            id,
            format!("{label} can be written at {}", path.display()),
        ));
    } else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!(
                "{label} parent directory is not writable: {}",
                parent.display()
            ),
            remediation,
        ));
    }
}

fn check_command(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    command: &'static str,
    message: &'static str,
    remediation: &'static str,
) {
    if command_exists(command) {
        checks.push(DoctorCheck::pass(section, id, message));
    } else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!("required command `{command}` was not found on PATH"),
            remediation,
        ));
    }
}

fn check_any_command(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    commands: &[&'static str],
    message: &'static str,
    remediation: &'static str,
) {
    if commands.iter().any(|command| command_exists(command)) {
        checks.push(DoctorCheck::pass(section, id, message));
    } else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!(
                "none of the required commands were found on PATH: {}",
                commands.join(", ")
            ),
            remediation,
        ));
    }
}

fn check_any_command_warn(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    commands: &[&'static str],
    message: &'static str,
    remediation: &'static str,
) {
    if commands.iter().any(|command| command_exists(command)) {
        checks.push(DoctorCheck::pass(section, id, message));
    } else {
        checks.push(DoctorCheck::warn(
            section,
            id,
            format!(
                "none of the fallback commands were found on PATH: {}",
                commands.join(", ")
            ),
            remediation,
        ));
    }
}

fn check_all_commands(
    checks: &mut Vec<DoctorCheck>,
    section: DoctorSection,
    id: &'static str,
    commands: &[&'static str],
    message: &'static str,
    remediation: &'static str,
) {
    let missing = commands
        .iter()
        .copied()
        .filter(|command| !command_exists(command))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        checks.push(DoctorCheck::pass(section, id, message));
    } else {
        checks.push(DoctorCheck::fail(
            section,
            id,
            format!(
                "required command(s) were not found on PATH: {}",
                missing.join(", ")
            ),
            remediation,
        ));
    }
}

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return is_executable_file(path);
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| is_executable_file(&dir.join(command)))
    })
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        path.metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn check_tui(options: &DoctorOptions, checks: &mut Vec<DoctorCheck>) {
    if options
        .walls_tui_preview
        .as_deref()
        .is_some_and(|value| matches!(value, "0" | "false" | "no" | "off"))
    {
        checks.push(DoctorCheck::warn(
            DoctorSection::Tui,
            "tui.preview",
            "TUI image preview is disabled by WALLS_TUI_PREVIEW",
            "unset WALLS_TUI_PREVIEW or use metadata preview mode intentionally",
        ));
    } else {
        checks.push(DoctorCheck::pass(
            DoctorSection::Tui,
            "tui.preview",
            "TUI preview is enabled or will fall back to metadata when unsupported",
        ));
    }
}

fn dir_is_writable(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let probe = path.join(".walls-doctor-write-test");
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PathsConfig, SourceEntry};
    use crate::ctx::WallsCtx;

    fn ctx_with_config(mut config: Config) -> (tempfile::TempDir, WallsCtx) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        config.paths = PathsConfig {
            cache_dir: root.join("cache").display().to_string(),
            download_dir: root.join("downloaded").display().to_string(),
            favorites_dir: root.join("favorites").display().to_string(),
            fetched_dir: root.join("fetched").display().to_string(),
            compose_dir: root.join("wallpaper").display().to_string(),
        };
        std::fs::write(
            root.join("config.json"),
            serde_json::to_string_pretty(&config).expect("config json"),
        )
        .expect("write config");
        std::fs::write(root.join("secrets.json"), "{}").expect("write secrets");
        let ctx = WallsCtx::load_from(root).expect("load ctx");
        (tmp, ctx)
    }

    fn folder_source() -> SourceEntry {
        SourceEntry {
            enabled: true,
            source_type: "folder".into(),
            label: None,
            path: None,
            query: None,
            url: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            api_key: None,
            image_path: None,
            title_path: None,
            sort: None,
            time: None,
        }
    }

    #[test]
    fn doctor_reports_ready_for_basic_local_config() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = false;
        let tmp_images = tempfile::tempdir().expect("images");
        std::fs::write(tmp_images.path().join("a.jpg"), b"x").expect("image");
        let mut source = folder_source();
        source.path = Some(tmp_images.path().display().to_string());
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);

        let report = run_doctor(&ctx, &DoctorOptions::default());

        assert!(report.ready, "{:#?}", report.checks);
        assert!(report.checks.iter().any(
            |check| check.id == "providers.local_sources" && check.status == DoctorStatus::Pass
        ));
        assert!(report.checks.iter().any(|check| {
            check.id == "providers.candidate_readiness" && check.status == DoctorStatus::Pass
        }));
    }

    #[test]
    fn doctor_fails_without_enabled_sources() {
        let mut config = crate::config::default_config().expect("default config");
        config.sources.clear();
        let (_tmp, ctx) = ctx_with_config(config);

        let report = run_doctor(&ctx, &DoctorOptions::default());

        assert!(!report.ready);
        assert!(report.checks.iter().any(|check| {
            check.id == "providers.enabled_sources" && check.status == DoctorStatus::Fail
        }));
    }

    #[test]
    fn doctor_warns_when_tui_preview_is_disabled() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = false;
        let tmp_images = tempfile::tempdir().expect("images");
        std::fs::write(tmp_images.path().join("a.jpg"), b"x").expect("image");
        let mut source = folder_source();
        source.path = Some(tmp_images.path().display().to_string());
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);
        let options = DoctorOptions {
            walls_tui_preview: Some("0".into()),
            ..DoctorOptions::default()
        };

        let report = run_doctor(&ctx, &options);

        assert!(report.ready);
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "tui.preview" && check.status == DoctorStatus::Warn));
    }

    #[test]
    fn doctor_fails_when_local_only_sources_have_no_candidates() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = false;
        let tmp_images = tempfile::tempdir().expect("images");
        let mut source = folder_source();
        source.path = Some(tmp_images.path().display().to_string());
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);

        let report = run_doctor(&ctx, &DoctorOptions::default());

        assert!(!report.ready);
        assert!(report.checks.iter().any(|check| {
            check.id == "providers.source_0.candidates" && check.status == DoctorStatus::Warn
        }));
        assert!(report.checks.iter().any(|check| {
            check.id == "providers.candidate_readiness" && check.status == DoctorStatus::Fail
        }));
    }

    #[test]
    fn doctor_warns_when_candidate_readiness_depends_on_online_providers() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = true;
        let mut source = folder_source();
        source.source_type = "bing".into();
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);

        let report = run_doctor(&ctx, &DoctorOptions::default());

        assert!(report.ready, "{:#?}", report.checks);
        assert!(report.checks.iter().any(|check| {
            check.id == "providers.candidate_readiness" && check.status == DoctorStatus::Warn
        }));
    }

    #[test]
    fn doctor_names_missing_source_credentials_in_recovery_guidance() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = true;
        let mut source = folder_source();
        source.source_type = "unsplash".into();
        source.query = Some("mountains".into());
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);

        let report = run_doctor(&ctx, &DoctorOptions::default());

        let check = report
            .checks
            .iter()
            .find(|check| check.id == "providers.source_0.credentials")
            .expect("credentials check");
        assert_eq!(check.status, DoctorStatus::Fail);
        let remediation = check.remediation.as_deref().expect("remediation");
        assert!(remediation.contains("`unsplash_access_key`"));
        assert!(remediation.contains("secrets.json"));
        assert!(remediation.contains("walls config validate"));
    }

    #[test]
    fn doctor_warns_when_download_storage_exceeds_quota() {
        let mut config = crate::config::default_config().expect("default config");
        config.change.internet_enabled = false;
        config.quota.enabled = true;
        config.quota.size_mb = 1;
        let tmp_images = tempfile::tempdir().expect("images");
        std::fs::write(tmp_images.path().join("a.jpg"), b"x").expect("image");
        let mut source = folder_source();
        source.path = Some(tmp_images.path().display().to_string());
        config.sources = vec![source];
        let (_tmp, ctx) = ctx_with_config(config);
        std::fs::write(
            ctx.paths.download_dir.join("large.jpg"),
            vec![0_u8; 2 * 1024 * 1024],
        )
        .expect("downloaded file");

        let report = run_doctor(&ctx, &DoctorOptions::default());

        assert!(report.ready, "{:#?}", report.checks);
        assert!(report.checks.iter().any(|check| {
            check.id == "storage.quota"
                && check.status == DoctorStatus::Warn
                && check
                    .remediation
                    .as_deref()
                    .is_some_and(|fix| fix.contains("walls cache prune --dry-run"))
        }));
    }

    #[test]
    fn command_exists_accepts_absolute_paths_without_using_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let command = tmp.path().join("helper");
        std::fs::write(&command, b"").expect("helper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755))
                .expect("helper permissions");
        }

        assert!(command_exists(command.to_str().expect("utf8 path")));
        assert!(!command_exists(
            tmp.path().join("missing").to_str().expect("utf8 path")
        ));
    }
}

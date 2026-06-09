use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::apply::{
    backend_setting_label, desktop_display_name, summarize_apply_environment_from_env,
};
use crate::autostart::{
    autostart_desktop_file_path, autostart_out_of_sync, tray_autostart_available,
    tray_autostart_enabled_for_desktop, AutostartSyncOpts,
};
use crate::config::{secrets_credential_present, source_secrets_key, SourceKind};
use crate::ctx::WallsCtx;
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
    check_storage_cache(ctx, &mut checks);
    check_tui(options, &mut checks);
    DoctorReport {
        ready: !checks
            .iter()
            .any(|check| check.status == DoctorStatus::Fail),
        checks,
    }
}

fn check_config(ctx: &WallsCtx, checks: &mut Vec<DoctorCheck>) {
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
                    "add the required key to secrets.json or disable this source",
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

    if ctx.config.quota.enabled && ctx.config.quota.size_mb == 0 {
        checks.push(DoctorCheck::warn(
            DoctorSection::StorageCache,
            "storage.quota",
            "download quota is enabled with a zero MiB limit",
            "set quota.size_mb above zero or disable quota",
        ));
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
}

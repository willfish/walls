use crate::config::UnsplashSourceConfig;
use crate::config::{
    wallhaven_resolution_choices, wallhaven_resolution_supported, ApplyBackendSetting, Config,
    Secrets, SourceEntry, SourceKind,
};
use crate::paths::{expand_home, WallsPaths};
use serde::Serialize;
use std::fmt;

const WALLHAVEN_SORTING_CHOICES: &[&str] = &[
    "date",
    "relevance",
    "random",
    "views",
    "favorites",
    "toplist",
];
const WALLHAVEN_ORDER_CHOICES: &[&str] = &["desc", "asc"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Error,
}

impl fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationDiagnostic {
    pub severity: ValidationSeverity,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ValidationDiagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.severity, self.path, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " (hint: {hint})")?;
        }
        Ok(())
    }
}

/// Log non-fatal config problems at load time (see also `walls config validate`).
pub fn warn_validation_issues(config: &Config, secrets: &Secrets, paths: &WallsPaths) {
    for issue in validate_config_diagnostics(config, secrets, paths) {
        tracing::warn!(%issue, "config validation");
    }
    for issue in secrets_file_permission_warnings(paths) {
        tracing::warn!(issue, "secrets file permissions");
    }
}

/// Full config check for `walls config validate` and non-blocking TUI warnings.
pub fn validate_config(config: &Config, secrets: &Secrets, paths: &WallsPaths) -> Vec<String> {
    validate_config_diagnostics(config, secrets, paths)
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect()
}

/// Full config check with structured diagnostics for CLI JSON output and future TUI grouping.
pub fn validate_config_diagnostics(
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
) -> Vec<ValidationDiagnostic> {
    let mut errors = Vec::new();

    if !paths.config_file.is_file() {
        errors.push(
            ValidationDiagnostic::error(
                "config",
                format!("config file not found: {}", paths.config_file.display()),
            )
            .with_hint("run `walls` once to create a default config, or create config.json"),
        );
    }

    for (index, src) in config.sources.iter().enumerate() {
        validate_source_entry(index, src, config, secrets, paths, &mut errors);
    }

    validate_wallhaven_provider(config, secrets, &mut errors);
    validate_apply_config(config, &mut errors);
    validate_quota_config(config, &mut errors);
    validate_tray_autostart(config, &mut errors);

    errors
}

/// Validate one source entry while editing it in the TUI (no global config checks).
pub fn validate_source_edit(
    index: usize,
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
) -> Vec<String> {
    validate_source_edit_diagnostics(index, config, secrets, paths)
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect()
}

pub fn validate_source_edit_diagnostics(
    index: usize,
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
) -> Vec<ValidationDiagnostic> {
    let mut errors = Vec::new();
    let Some(src) = config.sources.get(index) else {
        errors.push(ValidationDiagnostic::error(
            format!("sources[{index}]"),
            "source does not exist",
        ));
        return errors;
    };
    validate_source_entry(index, src, config, secrets, paths, &mut errors);
    errors
}

/// Validate the Wallhaven provider block while editing it in the TUI.
pub fn validate_wallhaven_edit(config: &Config, secrets: &Secrets) -> Vec<String> {
    validate_wallhaven_edit_diagnostics(config, secrets)
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect()
}

pub fn validate_wallhaven_edit_diagnostics(
    config: &Config,
    secrets: &Secrets,
) -> Vec<ValidationDiagnostic> {
    let mut errors = Vec::new();
    validate_wallhaven_provider(config, secrets, &mut errors);
    errors
}

fn validate_source_entry(
    index: usize,
    src: &SourceEntry,
    config: &Config,
    secrets: &Secrets,
    paths: &WallsPaths,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    if !src.enabled {
        return;
    }

    if src.source_type.trim().is_empty() {
        errors.push(
            ValidationDiagnostic::error(
                format!("sources[{index}].type"),
                format!(
                    "type is required for source {:?}",
                    src.label.as_deref().unwrap_or("(unnamed)")
                ),
            )
            .with_hint("set type to a supported source such as folder, reddit, unsplash, or json"),
        );
        return;
    }

    let source_kind = SourceKind::parse(&src.source_type);
    match source_kind {
        SourceKind::Folder | SourceKind::Image | SourceKind::Favorites | SourceKind::Fetched => {
            validate_local_source(index, src, source_kind, paths, errors);
        }
        _ => validate_provider_source(index, src, source_kind, config, secrets, errors),
    }
}

fn validate_local_source(
    index: usize,
    src: &SourceEntry,
    source_kind: SourceKind,
    paths: &WallsPaths,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    let expanded = match source_kind {
        SourceKind::Favorites => paths.favorites_dir.clone(),
        SourceKind::Fetched => paths.fetched_dir.clone(),
        _ => {
            let Some(path) = src.path.as_ref() else {
                errors.push(
                    ValidationDiagnostic::error(
                        format!("sources[{index}].path"),
                        format!(
                            "path is required for source {:?} with type {}",
                            src.label, src.source_type
                        ),
                    )
                    .with_hint("set path to a directory or image file that exists"),
                );
                return;
            };
            expand_home(path)
        }
    };
    if !expanded.exists() {
        errors.push(
            ValidationDiagnostic::error(
                format!("sources[{index}].path"),
                format!(
                    "path for source {:?} does not exist: {}",
                    src.label,
                    expanded.display()
                ),
            )
            .with_hint("create the path, correct the value, or disable this source"),
        );
    }
}

fn validate_provider_source(
    index: usize,
    src: &SourceEntry,
    source_kind: SourceKind,
    config: &Config,
    secrets: &Secrets,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    match source_kind {
        SourceKind::Unsplash => {
            if config.change.internet_enabled && secrets.unsplash_access_key.is_empty() {
                errors.push(
                    ValidationDiagnostic::error(
                        "secrets.unsplash_access_key",
                        "unsplash source is enabled but the access key is empty",
                    )
                    .with_hint("create an Unsplash application key or disable the Unsplash source"),
                );
            }
            if let Err(error) = UnsplashSourceConfig::from_source(src) {
                errors.push(ValidationDiagnostic::error(
                    format!("sources[{index}]"),
                    format!("source {:?}: {error}", src.label),
                ));
            }
            if let Some(orientation) = src.orientation.as_deref() {
                validate_choice(
                    &source_field(index, "orientation"),
                    orientation,
                    &["landscape", "portrait", "squarish"],
                    errors,
                );
            }
        }
        SourceKind::Reddit
            if config.change.internet_enabled && secrets.reddit_client_id.trim().is_empty() =>
        {
            errors.push(
                ValidationDiagnostic::error(
                    "secrets.reddit_client_id",
                    "reddit source is enabled but the client id is empty",
                )
                .with_hint(
                    "create a Reddit app at reddit.com/prefs/apps or disable the Reddit source",
                ),
            );
            validate_reddit_source(index, src, errors);
        }
        SourceKind::Reddit => validate_reddit_source(index, src, errors),
        SourceKind::Bing | SourceKind::Apod => {}
        SourceKind::Wallhaven | SourceKind::Weighting => {
            validate_required_text(index, src, "query", src.query.as_deref(), errors);
        }
        SourceKind::Json => validate_json_source(index, src, errors),
        SourceKind::MediaRss | SourceKind::Attribution => {
            validate_required_url(index, src, "url", src.url.as_deref(), errors);
        }
        SourceKind::Pixabay => {
            validate_required_text(index, src, "api_key", src.api_key.as_deref(), errors);
        }
        SourceKind::Immich => {
            validate_required_url(index, src, "url", src.url.as_deref(), errors);
            validate_required_text(index, src, "api_key", src.api_key.as_deref(), errors);
        }
        SourceKind::Spotlight => {
            if src
                .path
                .as_deref()
                .or(src.url.as_deref())
                .is_none_or(|path| path.trim().is_empty())
            {
                errors.push(
                    ValidationDiagnostic::error(
                        format!("sources[{index}].path"),
                        format!("spotlight source {:?} requires path or url", src.label),
                    )
                    .with_hint("set path to the Spotlight cache or provide a url"),
                );
            }
        }
        SourceKind::Unknown => {
            errors.push(
                ValidationDiagnostic::error(
                    format!("sources[{index}].type"),
                    format!(
                        "source {:?} has unsupported source type {:?}",
                        src.label, src.source_type
                    ),
                )
                .with_hint("set type to a supported source or disable this entry"),
            );
        }
        SourceKind::Folder | SourceKind::Image | SourceKind::Favorites | SourceKind::Fetched => {
            unreachable!("local source kinds are validated before provider schemas")
        }
    }
}

fn validate_json_source(index: usize, src: &SourceEntry, errors: &mut Vec<ValidationDiagnostic>) {
    validate_required_url(index, src, "url", src.url.as_deref(), errors);
    if let Some(image_path) = src.image_path.as_deref() {
        if !image_path.trim().starts_with("$.") && image_path.trim() != "$" {
            errors.push(
                ValidationDiagnostic::error(
                    source_field(index, "image_path"),
                    "image_path must be a JSON path starting with '$' or '$.'",
                )
                .with_hint("use a path such as $.download_url or leave it unset"),
            );
        }
    }
}

fn validate_reddit_source(index: usize, src: &SourceEntry, errors: &mut Vec<ValidationDiagnostic>) {
    validate_required_text(index, src, "query", src.query.as_deref(), errors);
    if let Some(sort) = src.sort.as_deref() {
        validate_choice(
            &source_field(index, "sort"),
            sort,
            crate::config::REDDIT_SORT_CHOICES,
            errors,
        );
    }
    if let Some(time) = src.time.as_deref() {
        validate_choice(
            &source_field(index, "time"),
            time,
            crate::config::REDDIT_TIME_CHOICES,
            errors,
        );
    }
}

fn validate_required_text(
    index: usize,
    src: &SourceEntry,
    field: &'static str,
    value: Option<&str>,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    if value.is_none_or(|value| value.trim().is_empty()) {
        errors.push(
            ValidationDiagnostic::error(
                source_field(index, field),
                format!(
                    "{field} is required for source {:?} with type {}",
                    src.label, src.source_type
                ),
            )
            .with_hint(format!("set {field} or disable this source")),
        );
    }
}

fn validate_required_url(
    index: usize,
    src: &SourceEntry,
    field: &'static str,
    value: Option<&str>,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        errors.push(
            ValidationDiagnostic::error(
                source_field(index, field),
                format!(
                    "{field} is required for source {:?} with type {}",
                    src.label, src.source_type
                ),
            )
            .with_hint(format!("set {field} to an http or https URL")),
        );
        return;
    };
    validate_http_url(&source_field(index, field), value, errors);
}

fn validate_http_url(field: &str, value: &str, errors: &mut Vec<ValidationDiagnostic>) {
    match reqwest::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {}
        Ok(url) => errors.push(
            ValidationDiagnostic::error(
                field,
                format!("must use http or https, got {}", url.scheme()),
            )
            .with_hint("replace the URL scheme with http or https"),
        ),
        Err(error) => errors.push(
            ValidationDiagnostic::error(field, format!("must be a valid URL: {error}"))
                .with_hint("set a complete URL such as https://example.com/feed.json"),
        ),
    }
}

fn source_field(index: usize, field: &str) -> String {
    format!("sources[{index}].{field}")
}

fn validate_wallhaven_provider(
    config: &Config,
    secrets: &Secrets,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    if !config.wallhaven.enabled {
        return;
    }

    let search = &config.wallhaven.search;
    validate_wallhaven_bitfield(
        "wallhaven.search.categories",
        &search.categories,
        true,
        errors,
    );
    validate_wallhaven_bitfield("wallhaven.search.purity", &search.purity, true, errors);

    if secrets.wallhaven_api_key.trim().is_empty()
        && search.purity.as_bytes().get(0..2) == Some(b"00")
        && search.purity.as_bytes().get(2) == Some(&b'1')
    {
        errors.push(
            ValidationDiagnostic::error(
                "wallhaven.search.purity",
                "cannot select only NSFW without secrets.wallhaven_api_key",
            )
            .with_hint(
                "add a Wallhaven API key or choose a purity value that includes non-NSFW results",
            ),
        );
    }

    validate_choice(
        "wallhaven.search.sorting",
        &search.sorting,
        WALLHAVEN_SORTING_CHOICES,
        errors,
    );
    validate_choice(
        "wallhaven.search.order",
        &search.order,
        WALLHAVEN_ORDER_CHOICES,
        errors,
    );
    validate_wallhaven_resolution("wallhaven.search.atleast", &search.atleast, errors);

    for (index, collection) in config.wallhaven.collections.iter().enumerate() {
        if collection.username.trim().is_empty() {
            errors.push(
                ValidationDiagnostic::error(
                    format!("wallhaven.collections[{index}].username"),
                    "must not be empty",
                )
                .with_hint("set the Wallhaven collection username or remove this collection"),
            );
        }
        if collection.id == 0 {
            errors.push(
                ValidationDiagnostic::error(
                    format!("wallhaven.collections[{index}].id"),
                    "must be greater than zero",
                )
                .with_hint("set the numeric Wallhaven collection id"),
            );
        }
    }
}

fn validate_wallhaven_bitfield(
    field: &str,
    value: &str,
    require_enabled_bit: bool,
    errors: &mut Vec<ValidationDiagnostic>,
) {
    if value.len() != 3 || !value.bytes().all(|byte| matches!(byte, b'0' | b'1')) {
        errors.push(
            ValidationDiagnostic::error(
                field,
                "must be three binary digits, for example 100 or 111",
            )
            .with_hint("use a three-character bitfield such as 100, 010, or 111"),
        );
        return;
    }

    if require_enabled_bit && value.bytes().all(|byte| byte == b'0') {
        errors.push(
            ValidationDiagnostic::error(field, "must enable at least one option")
                .with_hint("set at least one bit to 1"),
        );
    }
}

fn validate_choice(
    field: &str,
    value: &str,
    choices: &[&str],
    errors: &mut Vec<ValidationDiagnostic>,
) {
    if choices.contains(&value) {
        return;
    }

    errors.push(
        ValidationDiagnostic::error(field, format!("must be one of: {}", choices.join(", ")))
            .with_hint(format!("replace {value:?} with one of the listed values")),
    );
}

fn validate_wallhaven_resolution(field: &str, value: &str, errors: &mut Vec<ValidationDiagnostic>) {
    if wallhaven_resolution_supported(value) {
        return;
    }

    let choices = wallhaven_resolution_choices().join(", ");
    errors.push(
        ValidationDiagnostic::error(field, format!("must be one of: {choices}"))
            .with_hint(format!("choose Minimum resolution in the TUI, or replace {value:?} with one of the listed values")),
    );
}

fn validate_tray_autostart(config: &Config, errors: &mut Vec<ValidationDiagnostic>) {
    let Ok(config_home) = autostart_config_home() else {
        return;
    };
    let tray_bin = crate::bin_resolve::resolve_binary(crate::bin_resolve::BinResolveOpts {
        env_var: std::env::var("WALLS_TRAY_BIN").ok().as_deref(),
        current_exe: std::env::current_exe().ok().as_deref(),
        sibling_name: "walls-tray",
        build_default: None,
        path_fallback: "walls-tray",
    });
    let xdg_current_desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let xdg_session_desktop = std::env::var("XDG_SESSION_DESKTOP").ok();
    let desktop_startup_id = std::env::var("DESKTOP_STARTUP_ID").ok();
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();
    let opts = crate::autostart::AutostartSyncOpts {
        config_home: &config_home,
        tray_bin,
        config,
        xdg_current_desktop: xdg_current_desktop.as_deref(),
        xdg_session_desktop: xdg_session_desktop.as_deref(),
        desktop_startup_id: desktop_startup_id.as_deref(),
        xdg_session_type: xdg_session_type.as_deref(),
        wayland_display: wayland_display.as_deref(),
        display: display.as_deref(),
    };
    if crate::autostart::autostart_out_of_sync(&opts) {
        errors.push(
            ValidationDiagnostic::error(
                "tray.autostart",
                "tray autostart desktop entry is out of sync with config",
            )
            .with_hint("run `walls config sync`"),
        );
    }
}

fn autostart_config_home() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Ok(std::path::PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME")?;
    Ok(std::path::PathBuf::from(home).join(".config"))
}

fn validate_apply_config(config: &Config, errors: &mut Vec<ValidationDiagnostic>) {
    let custom_script = config
        .apply
        .custom_script
        .as_deref()
        .filter(|path| !path.trim().is_empty());

    match (config.apply.backend, custom_script) {
        (ApplyBackendSetting::CustomScript, Some(script)) => {
            let script_path = expand_home(script);
            if !script_path.is_file() {
                errors.push(
                    ValidationDiagnostic::error(
                        "apply.custom_script",
                        format!("not found or not a file: {}", script_path.display()),
                    )
                    .with_hint("set apply.custom_script to an existing executable file"),
                );
                return;
            }
            #[cfg(unix)]
            if !is_executable(&script_path) {
                errors.push(
                    ValidationDiagnostic::error(
                        "apply.custom_script",
                        format!("is not executable: {}", script_path.display()),
                    )
                    .with_hint(format!("run `chmod +x {}`", script_path.display())),
                );
            }
        }
        (ApplyBackendSetting::CustomScript, None) => {
            errors.push(
                ValidationDiagnostic::error(
                    "apply.custom_script",
                    "is required when apply.backend is custom-script",
                )
                .with_hint("set apply.custom_script or choose a different apply.backend"),
            );
        }
        (backend, Some(_)) => {
            errors.push(
                ValidationDiagnostic::error(
                    "apply.custom_script",
                    format!(
                        "is set but apply.backend is {}",
                        apply_backend_name(backend)
                    ),
                )
                .with_hint("set apply.backend to custom-script or remove apply.custom_script"),
            );
        }
        (_, None) => {}
    }

    if config.apply.backend == ApplyBackendSetting::Cosmic {
        let cosmic_path = expand_home(&config.apply.cosmic.config_path);
        if !cosmic_path.is_file() {
            errors.push(
                ValidationDiagnostic::error(
                    "apply.cosmic.config_path",
                    format!("not found: {}", cosmic_path.display()),
                )
                .with_hint("set the COSMIC config path or choose apply.backend auto"),
            );
        }
    }
}

fn validate_quota_config(config: &Config, errors: &mut Vec<ValidationDiagnostic>) {
    if config.quota.size_mb == 0 {
        errors.push(
            ValidationDiagnostic::error("quota.size_mb", "must be greater than zero")
                .with_hint("set quota.size_mb to a positive number of megabytes"),
        );
    }
}

fn apply_backend_name(backend: ApplyBackendSetting) -> &'static str {
    match backend {
        ApplyBackendSetting::Auto => "auto",
        ApplyBackendSetting::Cosmic => "cosmic",
        ApplyBackendSetting::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
        ApplyBackendSetting::Gnome => "gnome",
        ApplyBackendSetting::Kde => "kde",
        ApplyBackendSetting::Xfce => "xfce",
        ApplyBackendSetting::Sway => "sway",
        ApplyBackendSetting::Wlroots => "wlroots",
        ApplyBackendSetting::Hyprland => "hyprland",
        ApplyBackendSetting::Feh => "feh",
        ApplyBackendSetting::CustomScript => "custom-script",
    }
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[allow(
        clippy::verbose_bit_mask,
        reason = "custom script validation intentionally accepts any owner/group/other execute bit."
    )]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
}

#[cfg(unix)]
pub fn secrets_file_permission_warnings(paths: &WallsPaths) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(&paths.secrets_file) else {
        return Vec::new();
    };
    let mode = metadata.permissions().mode();
    #[allow(
        clippy::verbose_bit_mask,
        reason = "the secrets permission warning intentionally checks all group/other permission bits in one mode mask."
    )]
    if mode & 0o077 == 0 {
        return Vec::new();
    }

    vec![format!(
        "secrets file is readable by group or other users: {}; run `chmod 600 {}`",
        paths.secrets_file.display(),
        paths.secrets_file.display()
    )]
}

#[cfg(not(unix))]
pub fn secrets_file_permission_warnings(_paths: &WallsPaths) -> Vec<String> {
    Vec::new()
}

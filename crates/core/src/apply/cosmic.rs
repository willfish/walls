use std::fs;
use std::path::Path;
use std::process::Command;

use regex::Regex;

use crate::config::{ApplyConfig, CosmicApplyConfig, CosmicBackgroundEntryConfig, CosmicMethod};
use crate::paths::expand_home;

use super::fill_mode::FillMode;
use super::Applier;

fn escape_ron_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn collapse_commas_after_source_strip(contents: &str) -> String {
    let comma_re = Regex::new(r",\s*,").expect("valid regex");
    let open_comma_re = Regex::new(r"\(\s*,").expect("valid regex");
    let cleaned = comma_re.replace_all(contents, ",").into_owned();
    open_comma_re.replace_all(&cleaned, "(").into_owned()
}

fn insert_field_line(contents: &str, line: &str) -> String {
    if let Some(start) = contents.find("source: Path") {
        if let Some(rest) = contents[start..].find('\n') {
            let insert_at = start + rest + 1;
            let mut res = contents.to_string();
            res.insert_str(insert_at, &format!("    {line},\n"));
            return res;
        }
    }
    if let Some(close) = contents.rfind(')') {
        let mut res = contents.to_string();
        res.insert_str(close, &format!("    {line},\n"));
        return res;
    }
    contents.to_string()
}

fn upsert_u64_field(contents: &str, field: &str, value: u64) -> String {
    let re = Regex::new(&format!(r"{field}:\s*\d+")).expect("valid regex");
    let replacement = format!("{field}: {value}");
    if re.is_match(contents) {
        return re.replace_all(contents, replacement.as_str()).into_owned();
    }
    insert_field_line(contents, &replacement)
}

fn upsert_bool_field(contents: &str, field: &str, value: bool) -> String {
    let re = Regex::new(&format!(r"{field}:\s*(?:true|false)")).expect("valid regex");
    let replacement = format!("{field}: {value}");
    if re.is_match(contents) {
        return re.replace_all(contents, replacement.as_str()).into_owned();
    }
    insert_field_line(contents, &replacement)
}

fn patch_wallpaper_source(contents: &str, new_path: &Path) -> String {
    let escaped = escape_ron_path(new_path);
    let source_line = format!(r#"source: Path("{escaped}"),"#);

    // Remove all wallpaper path sources regardless of whitespace around `Path(`.
    let strip_re =
        Regex::new(r#"source:\s*Path\s*\(\s*"(?:\\.|[^"\\])*"\s*\)\s*,?\s*"#).expect("valid regex");
    let stripped = strip_re.replace_all(contents, "").into_owned();
    let cleaned = collapse_commas_after_source_strip(&stripped);

    // Prefer inserting after an output line (realistic COSMIC `all` / per-monitor tuples).
    let output_re = Regex::new(r#"(output:\s*"[^"]+"\s*,)"#).expect("valid regex for output");
    if let Some(caps) = output_re.captures(&cleaned) {
        let insert = format!("{}\n    {source_line}", &caps[1]);
        let (start, end) = {
            let m = caps.get(0).unwrap();
            (m.start(), m.end())
        };
        let mut res = cleaned;
        res.replace_range(start..end, &insert);
        return res;
    }

    // Fallback for `backgrounds: (` wrappers without an `output:` field yet.
    let insert_re = Regex::new(r"(backgrounds:\s*\(\s*)").expect("valid regex");
    if let Some(caps) = insert_re.captures(&cleaned) {
        let prefix = caps[1].to_string();
        let inserted = format!("{prefix}{source_line} ");
        let (start, end) = {
            let m = caps.get(0).unwrap();
            (m.start(), m.end())
        };
        let mut res = cleaned;
        res.replace_range(start..end, &inserted);
        return res;
    }

    // Last resort minimal (may lose other settings).
    format!(r"backgrounds: ( {source_line} color: [0.0, 0.0, 0.0, 1.0], )")
}

/// Patch the COSMIC background entry: wallpaper path plus managed slideshow fields.
pub fn patch_cosmic_background(
    contents: &str,
    new_path: &Path,
    entry: &CosmicBackgroundEntryConfig,
) -> String {
    let mut out = patch_wallpaper_source(contents, new_path);
    out = upsert_u64_field(&out, "rotation_frequency", entry.rotation_frequency);
    out = upsert_bool_field(&out, "filter_by_theme", entry.filter_by_theme);
    out
}

/// Patch `source: Path("...")` in COSMIC background RON config (Variety-compatible).
///
/// Also applies [`CosmicBackgroundEntryConfig::default`] so COSMIC slideshow stays off while
/// walls owns rotation.
pub fn patch_wallpaper_path(contents: &str, new_path: &Path) -> String {
    patch_cosmic_background(contents, new_path, &CosmicBackgroundEntryConfig::default())
}

fn default_cosmic_all_entry_template() -> String {
    r#"(
    output: "all",
    filter_by_theme: false,
    rotation_frequency: 0,
    filter_method: Lanczos,
    scaling_mode: Zoom,
    sampling_method: Alphanumeric,
)
"#
    .to_string()
}

pub struct CosmicConfigApplier {
    config_path: std::path::PathBuf,
    entry: CosmicBackgroundEntryConfig,
}

impl CosmicConfigApplier {
    pub fn new(cosmic: &CosmicApplyConfig) -> Self {
        Self {
            config_path: expand_home(&cosmic.config_path),
            entry: cosmic.entry.clone(),
        }
    }

    pub fn apply_path(&self, wallpaper: &Path) -> anyhow::Result<()> {
        let contents = if self.config_path.is_file() {
            fs::read_to_string(&self.config_path).map_err(|e| {
                anyhow::anyhow!(
                    "failed to read COSMIC background config {}: {e}",
                    self.config_path.display()
                )
            })?
        } else {
            if let Some(parent) = self.config_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    anyhow::anyhow!(
                        "failed to create COSMIC background config dir {}: {e}",
                        parent.display()
                    )
                })?;
            }
            tracing::info!(
                path = %self.config_path.display(),
                "COSMIC background config missing; creating managed entry"
            );
            default_cosmic_all_entry_template()
        };
        let patched = patch_cosmic_background(&contents, wallpaper, &self.entry);
        fs::write(&self.config_path, patched)?;
        tracing::info!(
            path = %self.config_path.display(),
            rotation_frequency = self.entry.rotation_frequency,
            filter_by_theme = self.entry.filter_by_theme,
            "patched COSMIC background entry"
        );

        // Best-effort force live update via ext ctl (if installed).
        let ctl_res = Command::new("cosmic-ext-bg-ctl")
            .arg("set")
            .arg(wallpaper)
            .status();
        if let Ok(status) = ctl_res {
            if status.success() {
                tracing::info!("also forced live bg via cosmic-ext-bg-ctl set");
            }
        }
        Ok(())
    }
}

pub struct CosmicExtBgApplier;

impl Applier for CosmicExtBgApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: super::fill_mode::ApplyTrigger,
    ) -> anyhow::Result<()> {
        let status = Command::new("cosmic-ext-bg-ctl")
            .arg("set")
            .arg(composed)
            .status()?;
        if !status.success() {
            anyhow::bail!("cosmic-ext-bg-ctl set failed with {status}");
        }
        Ok(())
    }
}

impl Applier for CosmicConfigApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: super::fill_mode::ApplyTrigger,
    ) -> anyhow::Result<()> {
        self.apply_path(composed)
    }
}

pub fn build_cosmic_applier(apply: &ApplyConfig) -> Box<dyn Applier> {
    match apply.cosmic.method {
        CosmicMethod::CosmicConfig => Box::new(CosmicConfigApplier::new(&apply.cosmic)),
        CosmicMethod::CosmicExtBgCtl => Box::new(CosmicExtBgApplier),
    }
}

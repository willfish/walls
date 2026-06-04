use std::path::Path;
use std::process::Command;

use super::fill_mode::{ApplyTrigger, FillMode};
use super::Applier;

pub struct GnomeApplier;

impl Applier for GnomeApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        for args in gnome_gsettings_commands(composed, fill) {
            let is_optional_dark_uri = args.get(2).is_some_and(|key| key == "picture-uri-dark");
            let status = Command::new("gsettings").args(&args).status()?;
            if !status.success() {
                if is_optional_dark_uri {
                    continue;
                }
                anyhow::bail!("gsettings failed setting GNOME wallpaper");
            }
        }
        Ok(())
    }
}

pub fn gnome_gsettings_commands(path: &Path, fill: FillMode) -> Vec<Vec<String>> {
    let uri = file_uri(path);
    let mut commands = vec![
        vec![
            "set".into(),
            "org.gnome.desktop.background".into(),
            "picture-uri".into(),
            uri.clone(),
        ],
        vec![
            "set".into(),
            "org.gnome.desktop.background".into(),
            "picture-uri-dark".into(),
            uri,
        ],
    ];

    if let Some(option) = fill.gnome_picture_options() {
        commands.push(vec![
            "set".into(),
            "org.gnome.desktop.background".into(),
            "picture-options".into(),
            option.into(),
        ]);
    }

    commands
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", percent_encode_path(path))
}

#[cfg(unix)]
fn percent_encode_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    percent_encode_bytes(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn percent_encode_path(path: &Path) -> String {
    percent_encode_bytes(path.to_string_lossy().as_bytes())
}

fn percent_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

use std::path::Path;
use std::process::Command;

use super::file_uri::file_uri;
use super::fill_mode::{ApplyTrigger, FillMode};
use super::Applier;

pub struct KdeApplier;

impl Applier for KdeApplier {
    fn set_wallpaper(
        &self,
        composed: &Path,
        _original: &Path,
        _fill: FillMode,
        _trigger: ApplyTrigger,
    ) -> anyhow::Result<()> {
        let output = Command::new("dbus-send")
            .args(kde_dbus_send_args(composed))
            .output()?;

        if !output.status.success() {
            anyhow::bail!("dbus-send failed setting KDE Plasma wallpaper");
        }

        if let Some(unsupported) = unsupported_plugins_from_dbus_reply(&output.stdout) {
            if !unsupported.trim().is_empty() {
                anyhow::bail!("unsupported KDE wallpaper plugin(s): {unsupported}");
            }
        }

        Ok(())
    }
}

pub fn kde_dbus_send_args(path: &Path) -> Vec<String> {
    vec![
        "--print-reply".into(),
        "--type=method_call".into(),
        "--dest=org.kde.plasmashell".into(),
        "/PlasmaShell".into(),
        "org.kde.PlasmaShell.evaluateScript".into(),
        format!("string:{}", plasma_script(path)),
    ]
}

pub fn plasma_script(path: &Path) -> String {
    let uri = file_uri(path);
    format!(
        r#"
        let supportedPlugins = Array('org.kde.image', 'a2n.blur');
        let unsupportedPlugins = [];
        let allDesktops = desktops();
        for (let d of allDesktops) {{
            if (supportedPlugins.includes(d.wallpaperPlugin)) {{
                d.currentConfigGroup = Array('Wallpaper', d.wallpaperPlugin, 'General');
                d.writeConfig('Image', "{uri}");
            }}
            else if (!unsupportedPlugins.includes(d.wallpaperPlugin)) {{
                unsupportedPlugins.push(d.wallpaperPlugin);
            }}
        }}
        print(unsupportedPlugins);
    "#
    )
}

pub fn unsupported_plugins_from_dbus_reply(stdout: &[u8]) -> Option<String> {
    let reply = std::str::from_utf8(stdout).ok()?;
    let start = reply.find("string \"")? + "string \"".len();
    let rest = &reply[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

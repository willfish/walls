use std::path::Path;
use std::process::Command;

use anyhow::Context;

pub fn spawn_tui(walls: &Path) -> anyhow::Result<()> {
    let override_cmd = std::env::var("WALLS_TUI_CMD").ok();
    let terminal = std::env::var("TERMINAL").ok();
    let xdg_terminal_exec = xdg_terminal_exec_on_path();
    let command = tui_command(
        walls,
        override_cmd.as_deref(),
        terminal.as_deref(),
        xdg_terminal_exec.as_deref(),
    )?;
    Command::new(&command.program)
        .args(&command.args)
        .spawn()
        .with_context(|| command.recovery_context())?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TuiCommand {
    program: String,
    args: Vec<String>,
    source: &'static str,
}

impl TuiCommand {
    fn recovery_context(&self) -> String {
        format!(
            "failed to launch TUI via {}: {}",
            self.source,
            self.display_command()
        )
    }

    fn display_command(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(crate) fn tui_command(
    walls: &Path,
    override_cmd: Option<&str>,
    terminal: Option<&str>,
    xdg_terminal_exec: Option<&str>,
) -> anyhow::Result<TuiCommand> {
    if let Some(cmd) = override_cmd {
        return Ok(TuiCommand {
            program: "sh".into(),
            args: vec![
                "-c".into(),
                cmd.replace("{walls}", &walls.display().to_string()),
            ],
            source: "WALLS_TUI_CMD",
        });
    }

    let walls_str = walls
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("walls path is not valid UTF-8"))?;

    if let Some(xdg_terminal_exec) = xdg_terminal_exec {
        return Ok(TuiCommand {
            program: xdg_terminal_exec.into(),
            args: vec!["--app-id=walls".into(), walls_str.into(), "tui".into()],
            source: "xdg-terminal-exec",
        });
    }

    if let Some(terminal) = terminal {
        if terminal_basename(terminal) == "ghostty" {
            return Ok(TuiCommand {
                program: terminal.into(),
                args: vec![
                    "--class=walls".into(),
                    "-e".into(),
                    walls_str.into(),
                    "tui".into(),
                ],
                source: "TERMINAL",
            });
        }
        return Ok(TuiCommand {
            program: terminal.into(),
            args: vec!["-e".into(), walls_str.into(), "tui".into()],
            source: "TERMINAL",
        });
    }

    Ok(TuiCommand {
        program: "alacritty".into(),
        args: vec!["-e".into(), walls_str.into(), "tui".into()],
        source: "alacritty fallback",
    })
}

fn terminal_basename(terminal: &str) -> &str {
    Path::new(terminal)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(terminal)
}

fn xdg_terminal_exec_on_path() -> Option<String> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("xdg-terminal-exec");
            candidate.is_file().then(|| candidate.display().to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_command_uses_override_and_substitutes_walls_path() {
        let command = tui_command(
            Path::new("/opt/walls/bin/walls"),
            Some("kitty -- {walls} tui"),
            Some("wezterm"),
            Some("/usr/bin/xdg-terminal-exec"),
        )
        .unwrap();

        assert_eq!(command.program, "sh");
        assert_eq!(
            command.args,
            vec!["-c", "kitty -- /opt/walls/bin/walls tui"]
        );
        assert_eq!(command.source, "WALLS_TUI_CMD");
    }

    #[test]
    fn tui_command_uses_xdg_terminal_exec_before_terminal_env() {
        let command = tui_command(
            Path::new("/opt/walls/bin/walls"),
            None,
            Some("foot"),
            Some("/usr/bin/xdg-terminal-exec"),
        )
        .unwrap();

        assert_eq!(command.program, "/usr/bin/xdg-terminal-exec");
        assert_eq!(
            command.args,
            vec!["--app-id=walls", "/opt/walls/bin/walls", "tui"]
        );
        assert_eq!(command.source, "xdg-terminal-exec");
    }

    #[test]
    fn tui_command_uses_terminal_env_when_desktop_wrapper_is_missing() {
        let command =
            tui_command(Path::new("/opt/walls/bin/walls"), None, Some("foot"), None).unwrap();

        assert_eq!(command.program, "foot");
        assert_eq!(command.args, vec!["-e", "/opt/walls/bin/walls", "tui"]);
        assert_eq!(command.source, "TERMINAL");
    }

    #[test]
    fn tui_command_preserves_app_id_for_ghostty_terminal() {
        let command = tui_command(
            Path::new("/opt/walls/bin/walls"),
            None,
            Some("/usr/bin/ghostty"),
            None,
        )
        .unwrap();

        assert_eq!(command.program, "/usr/bin/ghostty");
        assert_eq!(
            command.args,
            vec!["--class=walls", "-e", "/opt/walls/bin/walls", "tui"]
        );
        assert_eq!(command.source, "TERMINAL");
    }

    #[test]
    fn tui_command_uses_xdg_terminal_exec_before_alacritty_fallback() {
        let command = tui_command(
            Path::new("/opt/walls/bin/walls"),
            None,
            None,
            Some("/usr/bin/xdg-terminal-exec"),
        )
        .unwrap();

        assert_eq!(command.program, "/usr/bin/xdg-terminal-exec");
        assert_eq!(
            command.args,
            vec!["--app-id=walls", "/opt/walls/bin/walls", "tui"]
        );
        assert_eq!(command.source, "xdg-terminal-exec");
    }

    #[test]
    fn tui_command_defaults_terminal_to_alacritty() {
        let command = tui_command(Path::new("/opt/walls/bin/walls"), None, None, None).unwrap();

        assert_eq!(command.program, "alacritty");
        assert_eq!(command.args, vec!["-e", "/opt/walls/bin/walls", "tui"]);
        assert_eq!(command.source, "alacritty fallback");
    }

    #[test]
    fn tui_command_recovery_context_names_source_and_command() {
        let command = tui_command(
            Path::new("/opt/walls/bin/walls"),
            None,
            Some("/usr/bin/ghostty"),
            None,
        )
        .unwrap();

        let context = command.recovery_context();

        assert!(context.contains("via TERMINAL"), "{context}");
        assert!(
            context.contains("/usr/bin/ghostty --class=walls -e /opt/walls/bin/walls tui"),
            "{context}"
        );
    }
}

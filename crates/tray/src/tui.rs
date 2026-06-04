use std::path::Path;
use std::process::Command;

pub fn spawn_tui(walls: &Path) -> anyhow::Result<()> {
    let override_cmd = std::env::var("WALLS_TUI_CMD").ok();
    let terminal = std::env::var("TERMINAL").ok();
    let command = tui_command(walls, override_cmd.as_deref(), terminal.as_deref())?;
    Command::new(&command.program).args(&command.args).spawn()?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TuiCommand {
    program: String,
    args: Vec<String>,
}

pub(crate) fn tui_command(
    walls: &Path,
    override_cmd: Option<&str>,
    terminal: Option<&str>,
) -> anyhow::Result<TuiCommand> {
    if let Some(cmd) = override_cmd {
        return Ok(TuiCommand {
            program: "sh".into(),
            args: vec![
                "-c".into(),
                cmd.replace("{walls}", &walls.display().to_string()),
            ],
        });
    }

    let terminal = terminal.unwrap_or("alacritty");
    let walls_str = walls
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("walls path is not valid UTF-8"))?;
    Ok(TuiCommand {
        program: terminal.into(),
        args: vec!["-e".into(), walls_str.into(), "tui".into()],
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
        )
        .unwrap();

        assert_eq!(command.program, "sh");
        assert_eq!(
            command.args,
            vec!["-c", "kitty -- /opt/walls/bin/walls tui"]
        );
    }

    #[test]
    fn tui_command_defaults_to_terminal_exec() {
        let command = tui_command(Path::new("/opt/walls/bin/walls"), None, Some("foot")).unwrap();

        assert_eq!(command.program, "foot");
        assert_eq!(command.args, vec!["-e", "/opt/walls/bin/walls", "tui"]);
    }

    #[test]
    fn tui_command_defaults_terminal_to_alacritty() {
        let command = tui_command(Path::new("/opt/walls/bin/walls"), None, None).unwrap();

        assert_eq!(command.program, "alacritty");
        assert_eq!(command.args, vec!["-e", "/opt/walls/bin/walls", "tui"]);
    }
}

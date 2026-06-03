use std::path::Path;
use std::process::Command;

pub fn spawn_tui(walls: &Path) -> anyhow::Result<()> {
    if let Ok(cmd) = std::env::var("WALLS_TUI_CMD") {
        let script = cmd.replace("{walls}", &walls.display().to_string());
        Command::new("sh").arg("-c").arg(script).spawn()?;
        return Ok(());
    }
    let terminal = std::env::var("TERMINAL").unwrap_or_else(|_| "alacritty".into());
    let walls_str = walls
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("walls path is not valid UTF-8"))?;
    Command::new(terminal)
        .args(["-e", walls_str, "tui"])
        .spawn()?;
    Ok(())
}

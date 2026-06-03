use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tracing_subscriber::EnvFilter;

fn walls_bin() -> PathBuf {
    if let Ok(p) = std::env::var("WALLS_BIN") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.parent().unwrap().join("walls");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("walls")
}

fn run_walls(args: &[&str]) -> anyhow::Result<()> {
    let bin = walls_bin();
    let status = Command::new(&bin).args(args).status()?;
    if !status.success() {
        anyhow::bail!("{} {} failed: {status}", bin.display(), args.join(" "));
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walls_tray=info".parse()?))
        .init();

    let menu = Menu::new();
    let next = MenuItem::new("Next wallpaper", true, None);
    let prev = MenuItem::new("Previous wallpaper", true, None);
    let pause = MenuItem::new("Toggle pause", true, None);
    let quit = MenuItem::new("Quit tray", true, None);
    menu.append(&next)?;
    menu.append(&prev)?;
    menu.append(&pause)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    let icon = tray_icon::Icon::from_rgba(vec![80, 120, 200, 255], 1, 1)?;
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("walls")
        .with_icon(icon)
        .build()?;

    let menu_channel = MenuEvent::receiver();
    loop {
        if let Ok(event) = menu_channel.recv() {
            let id = event.id().0.clone();
            if id == next.id().0 {
                let _ = run_walls(&["next"]);
            } else if id == prev.id().0 {
                let _ = run_walls(&["prev"]);
            } else if id == pause.id().0 {
                let _ = run_walls(&["toggle-pause"]);
            } else if id == quit.id().0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

mod icon;
mod tui;

use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use tracing_subscriber::EnvFilter;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

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

fn run_walls(walls: &PathBuf, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new(walls).args(args).status()?;
    if !status.success() {
        anyhow::bail!("{} {} failed: {status}", walls.display(), args.join(" "));
    }
    Ok(())
}

fn refresh_tray(tray: &tray_icon::TrayIcon) {
    if let Ok(icon) = icon::icon_from_state() {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(&icon::tooltip_from_state()));
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walls_tray=info".parse()?))
        .init();

    let walls = walls_bin();
    let menu = Menu::new();
    let next = MenuItem::new("Next wallpaper", true, None);
    let prev = MenuItem::new("Previous wallpaper", true, None);
    let pause = MenuItem::new("Toggle pause", true, None);
    let open_tui = MenuItem::new("Open TUI", true, None);
    let quit = MenuItem::new("Quit tray", true, None);
    menu.append(&next)?;
    menu.append(&prev)?;
    menu.append(&pause)?;
    menu.append(&open_tui)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    let icon = icon::icon_from_state()
        .unwrap_or_else(|_| icon::default_icon().expect("default tray icon"));
    let tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(icon::tooltip_from_state())
        .with_icon(icon)
        .build()?;

    let menu_channel = MenuEvent::receiver();
    loop {
        if let Ok(event) = menu_channel.recv() {
            let id = event.id().0.clone();
            if id == next.id().0 {
                let _ = run_walls(&walls, &["next"]);
                refresh_tray(&tray);
            } else if id == prev.id().0 {
                let _ = run_walls(&walls, &["prev"]);
                refresh_tray(&tray);
            } else if id == pause.id().0 {
                let _ = run_walls(&walls, &["toggle-pause"]);
                refresh_tray(&tray);
            } else if id == open_tui.id().0 {
                let _ = tui::spawn_tui(&walls);
            } else if id == quit.id().0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

//! Legacy AppIndicator tray (X11 and desktops with GTK tray host).

use std::thread;
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::icon;
use crate::rotation::RotationLoop;
use crate::state_watch::StateWatcher;
use crate::{resolve_walls_bin, run_walls, WallsCommand};

pub fn run() -> anyhow::Result<()> {
    tracing::info!(
        "walls-tray using walls binary at {}",
        resolve_walls_bin().display()
    );

    run_loop()
}

fn run_loop() -> anyhow::Result<()> {
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

    let icon = icon::appindicator_icon_from_state()
        .unwrap_or_else(|_| icon::default_appindicator_icon().expect("default tray icon"));
    let tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(icon::tooltip_from_state())
        .with_icon(icon)
        .build()?;

    tracing::info!("walls-tray running via AppIndicator");

    let menu_channel = MenuEvent::receiver();
    let mut watcher = StateWatcher::new().ok();
    let mut rotation = RotationLoop::new();
    let mut last_poll = std::time::Instant::now();
    const POLL_INTERVAL: Duration = Duration::from_millis(200);

    loop {
        if last_poll.elapsed() >= POLL_INTERVAL {
            last_poll = std::time::Instant::now();
            rotation.poll();
            if watcher.as_mut().is_some_and(|watcher| watcher.poll()) {
                refresh_tray(&tray);
            }
        }

        if let Ok(event) = menu_channel.recv() {
            let walls = resolve_walls_bin();
            let id = event.id().0.clone();
            if id == next.id().0 {
                crate::rotation::advance_manual();
                refresh_tray(&tray);
            } else if id == prev.id().0 {
                let _ = run_walls(&walls, WallsCommand::Prev.args());
                refresh_tray(&tray);
            } else if id == pause.id().0 {
                let _ = run_walls(&walls, WallsCommand::TogglePause.args());
                refresh_tray(&tray);
            } else if id == open_tui.id().0 {
                let _ = crate::tui::spawn_tui(&walls);
            } else if id == quit.id().0 {
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn refresh_tray(tray: &tray_icon::TrayIcon) {
    if let Ok(icon) = icon::appindicator_icon_from_state() {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(&icon::tooltip_from_state()));
}

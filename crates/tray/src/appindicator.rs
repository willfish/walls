//! Legacy AppIndicator tray (X11 and desktops with GTK tray host).

use std::thread;
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::actions::{dispatch, menu_actions, MenuAction};
use crate::icon;
use crate::resolve_walls_bin;
use crate::rotation::RotationLoop;
use crate::state_watch::StateWatcher;

pub fn run() -> anyhow::Result<()> {
    tracing::info!(
        "walls-tray using walls binary at {}",
        resolve_walls_bin().display()
    );

    run_loop()
}

fn run_loop() -> anyhow::Result<()> {
    let menu = Menu::new();
    let mut action_items = Vec::new();
    for spec in menu_actions() {
        if spec.separator_before {
            menu.append(&PredefinedMenuItem::separator())?;
        }
        let item = MenuItem::new(spec.label, true, None);
        menu.append(&item)?;
        action_items.push((item, spec.action));
    }

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
            if let Some(action) = action_for_menu_id(event.id().0.as_str(), &action_items) {
                let outcome = dispatch(action);
                if outcome.quit {
                    break;
                }
                if outcome.refresh {
                    refresh_tray(&tray);
                }
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

fn action_for_menu_id(id: &str, items: &[(MenuItem, MenuAction)]) -> Option<MenuAction> {
    items
        .iter()
        .find_map(|(item, action)| (item.id().0 == id).then_some(*action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_lookup_maps_menu_ids_to_shared_actions() {
        let next = MenuItem::new("Next wallpaper", true, None);
        let quit = MenuItem::new("Quit tray", true, None);
        let items = vec![
            (next.clone(), MenuAction::Next),
            (quit.clone(), MenuAction::Quit),
        ];

        assert_eq!(
            action_for_menu_id(next.id().0.as_str(), &items),
            Some(MenuAction::Next)
        );
        assert_eq!(
            action_for_menu_id(quit.id().0.as_str(), &items),
            Some(MenuAction::Quit)
        );
        assert_eq!(action_for_menu_id("missing", &items), None);
    }
}

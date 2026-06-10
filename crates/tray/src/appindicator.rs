//! Legacy AppIndicator tray (X11 and desktops with GTK tray host).

use std::thread;
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::actions::{dispatch, menu_actions, tooltip_with_feedback, ActionFeedback, MenuAction};
use crate::icon;
use crate::preview_prewarm::PreviewPrewarmer;
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
    let (menu, mut action_items) = build_menu()?;

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
    let mut preview_prewarm = PreviewPrewarmer::new().ok();
    let mut rotation = RotationLoop::new();
    let mut last_poll = std::time::Instant::now();
    const POLL_INTERVAL: Duration = Duration::from_millis(200);

    loop {
        if last_poll.elapsed() >= POLL_INTERVAL {
            last_poll = std::time::Instant::now();
            rotation.poll();
            if let Some(prewarm) = preview_prewarm.as_mut() {
                prewarm.poll();
            }
            if watcher.as_mut().is_some_and(|watcher| watcher.poll()) {
                refresh_tray(&tray, None, &mut action_items);
            }
        }

        if let Ok(event) = menu_channel.recv() {
            if let Some(action) = action_for_menu_id(event.id().0.as_str(), &action_items) {
                let outcome = dispatch(action);
                if outcome.quit {
                    break;
                }
                if outcome.refresh {
                    refresh_tray(&tray, outcome.feedback.as_ref(), &mut action_items);
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn build_menu() -> anyhow::Result<(Menu, Vec<(MenuItem, MenuAction)>)> {
    let menu = Menu::new();
    let mut action_items = Vec::new();
    for spec in menu_actions() {
        if spec.separator_before {
            menu.append(&PredefinedMenuItem::separator())?;
        }
        let item = MenuItem::new(spec.label.as_ref(), spec.enabled, None);
        menu.append(&item)?;
        if let Some(action) = spec.action {
            action_items.push((item, action));
        }
    }
    Ok((menu, action_items))
}

fn refresh_tray(
    tray: &tray_icon::TrayIcon,
    feedback: Option<&ActionFeedback>,
    action_items: &mut Vec<(MenuItem, MenuAction)>,
) {
    if let Ok(icon) = icon::appindicator_icon_from_state() {
        let _ = tray.set_icon(Some(icon));
    }
    let tooltip = tooltip_with_feedback(&icon::tooltip_from_state(), feedback);
    let _ = tray.set_tooltip(Some(&tooltip));
    match build_menu() {
        Ok((menu, items)) => {
            tray.set_menu(Some(Box::new(menu)));
            *action_items = items;
        }
        Err(err) => tracing::warn!("refresh tray menu failed: {err:#}"),
    }
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

    #[cfg_attr(
        target_os = "macos",
        ignore = "muda::Menu must be created on the macOS main thread"
    )]
    #[test]
    fn build_menu_uses_shared_action_order_and_labels() {
        let (_menu, items) = build_menu().expect("build menu");
        let specs = menu_actions();

        assert_eq!(
            items.len(),
            specs.iter().filter(|spec| spec.action.is_some()).count()
        );
        for ((item, action), spec) in items
            .iter()
            .zip(specs.iter().filter(|spec| spec.action.is_some()))
        {
            assert_eq!(Some(*action), spec.action);
            if !matches!(spec.action, Some(MenuAction::Pause | MenuAction::Resume)) {
                assert_eq!(item.text(), spec.label);
            }
        }
    }
}

//! StatusNotifierItem tray (Wayland-native; COSMIC, KDE Plasma, etc.).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{MenuItem, Orientation, Tray};

use crate::actions::{dispatch, menu_actions, tooltip_with_feedback, ActionFeedback, MenuAction};
use crate::icon;
use crate::preview_prewarm::PreviewPrewarmer;
use crate::resolve_walls_bin;
use crate::rotation::RotationLoop;
use crate::state_watch::StateWatcher;

pub struct WallsSniTray {
    tooltip: String,
    icons: Vec<ksni::Icon>,
    feedback: Option<ActionFeedback>,
    action_tx: Sender<MenuAction>,
}

impl WallsSniTray {
    pub fn new(action_tx: Sender<MenuAction>) -> Self {
        let mut tray = Self {
            tooltip: icon::tooltip_from_state(),
            icons: icon::ksni_icons_from_state(),
            feedback: None,
            action_tx,
        };
        if tray.icons.is_empty() {
            tray.icons = icon::default_ksni_icons();
        }
        tray
    }

    pub fn refresh_state(&mut self) {
        self.tooltip = icon::tooltip_from_state();
        self.icons = icon::ksni_icons_from_state();
        if self.icons.is_empty() {
            self.icons = icon::default_ksni_icons();
        }
    }

    pub fn set_feedback(&mut self, feedback: Option<ActionFeedback>) {
        self.feedback = feedback;
        self.refresh_state();
    }

    fn item(label: &str, action: MenuAction, tx: Sender<MenuAction>) -> MenuItem<Self> {
        StandardItem {
            label: label.into(),
            activate: Box::new(move |_| {
                if tx.send(action).is_err() {
                    tracing::warn!("tray action channel closed");
                }
            }),
            ..Default::default()
        }
        .into()
    }

    fn send_action(&self, action: MenuAction) {
        if self.action_tx.send(action).is_err() {
            tracing::warn!("tray action channel closed");
        }
    }
}

fn action_for_scroll(delta: i32, orientation: Orientation) -> Option<MenuAction> {
    match (delta.cmp(&0), orientation) {
        (std::cmp::Ordering::Greater, Orientation::Vertical) => Some(MenuAction::Next),
        (std::cmp::Ordering::Less, Orientation::Vertical) => Some(MenuAction::Prev),
        _ => None,
    }
}

impl Tray for WallsSniTray {
    fn id(&self) -> String {
        "walls".into()
    }

    fn title(&self) -> String {
        tooltip_with_feedback(&self.tooltip, self.feedback.as_ref())
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx = self.action_tx.clone();
        let mut items = Vec::new();
        for spec in menu_actions() {
            if spec.separator_before {
                items.push(MenuItem::Separator);
            }
            items.push(Self::item(&spec.label, spec.action, tx.clone()));
        }
        items
    }

    fn scroll(&mut self, delta: i32, orientation: Orientation) {
        if let Some(action) = action_for_scroll(delta, orientation) {
            self.send_action(action);
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    tracing::info!(
        "walls-tray using walls binary at {}",
        resolve_walls_bin().display()
    );

    let quit = Arc::new(AtomicBool::new(false));
    let (action_tx, action_rx) = mpsc::channel();
    let tray = WallsSniTray::new(action_tx);
    let handle = tray
        .spawn()
        .map_err(|err| anyhow::anyhow!("SNI tray failed: {err}"))?;
    tracing::info!("walls-tray running via StatusNotifierItem");

    let worker_quit = quit.clone();
    let worker_handle = handle.clone();
    thread::spawn(move || {
        while let Ok(action) = action_rx.recv() {
            let outcome = dispatch(action);
            if outcome.quit {
                worker_quit.store(true, Ordering::Relaxed);
                break;
            }
            if outcome.refresh {
                let feedback = outcome.feedback;
                let _ = worker_handle.update(|tray: &mut WallsSniTray| {
                    tray.set_feedback(feedback);
                });
            }
        }
    });

    let poll_handle = handle.clone();
    let mut watcher = StateWatcher::new().ok();
    let mut preview_prewarm = PreviewPrewarmer::new().ok();
    let mut rotation = RotationLoop::new();
    while !quit.load(Ordering::Relaxed) {
        rotation.poll();
        if let Some(prewarm) = preview_prewarm.as_mut() {
            prewarm.poll();
        }
        if watcher.as_mut().is_some_and(|watcher| watcher.poll()) {
            let _ = poll_handle.update(|tray: &mut WallsSniTray| tray.refresh_state());
        }
        thread::sleep(Duration::from_millis(200));
    }

    handle.shutdown().wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::FeedbackKind;

    #[test]
    fn vertical_scroll_maps_to_next_and_previous_actions() {
        assert_eq!(
            action_for_scroll(1, Orientation::Vertical),
            Some(MenuAction::Next)
        );
        assert_eq!(
            action_for_scroll(120, Orientation::Vertical),
            Some(MenuAction::Next)
        );
        assert_eq!(
            action_for_scroll(-1, Orientation::Vertical),
            Some(MenuAction::Prev)
        );
        assert_eq!(
            action_for_scroll(-120, Orientation::Vertical),
            Some(MenuAction::Prev)
        );
    }

    #[test]
    fn scroll_ignores_zero_and_horizontal_events() {
        assert_eq!(action_for_scroll(0, Orientation::Vertical), None);
        assert_eq!(action_for_scroll(1, Orientation::Horizontal), None);
        assert_eq!(action_for_scroll(-1, Orientation::Horizontal), None);
    }

    #[test]
    fn tray_trait_exposes_stable_identity_title_icon_and_menu() {
        let (tx, _rx) = mpsc::channel();
        let mut tray = WallsSniTray::new(tx);

        assert_eq!(tray.id(), "walls");
        assert!(!tray.title().is_empty());
        assert!(!tray.icon_pixmap().is_empty());

        tray.set_feedback(Some(ActionFeedback {
            kind: FeedbackKind::Success,
            message: "Changed wallpaper".into(),
        }));

        assert!(tray.title().contains("Changed wallpaper"));

        let menu = tray.menu();
        assert_eq!(
            menu.len(),
            menu_actions().len()
                + menu_actions()
                    .iter()
                    .filter(|spec| spec.separator_before)
                    .count()
        );
    }

    #[test]
    fn scroll_sends_actions_through_tray_channel() {
        let (tx, rx) = mpsc::channel();
        let mut tray = WallsSniTray::new(tx);

        tray.scroll(1, Orientation::Vertical);
        tray.scroll(-1, Orientation::Vertical);
        tray.scroll(1, Orientation::Horizontal);

        assert_eq!(rx.recv().expect("next action"), MenuAction::Next);
        assert_eq!(rx.recv().expect("previous action"), MenuAction::Prev);
        assert!(rx.try_recv().is_err());
    }
}

//! StatusNotifierItem tray (Wayland-native; COSMIC, KDE Plasma, etc.).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{MenuItem, Tray};

use crate::actions::{dispatch, MenuAction};
use crate::icon;
use crate::resolve_walls_bin;
use crate::rotation::RotationLoop;
use crate::state_watch::StateWatcher;

pub struct WallsSniTray {
    tooltip: String,
    icons: Vec<ksni::Icon>,
    action_tx: Sender<MenuAction>,
}

impl WallsSniTray {
    pub fn new(action_tx: Sender<MenuAction>) -> Self {
        let mut tray = Self {
            tooltip: icon::tooltip_from_state(),
            icons: icon::ksni_icons_from_state(),
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
}

impl Tray for WallsSniTray {
    fn id(&self) -> String {
        "walls".into()
    }

    fn title(&self) -> String {
        self.tooltip.clone()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx = self.action_tx.clone();
        vec![
            Self::item("Next wallpaper", MenuAction::Next, tx.clone()),
            Self::item("Previous wallpaper", MenuAction::Prev, tx.clone()),
            Self::item("Toggle pause", MenuAction::TogglePause, tx.clone()),
            Self::item("Open TUI", MenuAction::OpenTui, tx.clone()),
            MenuItem::Separator,
            StandardItem {
                label: "Quit tray".into(),
                activate: Box::new({
                    let tx = tx.clone();
                    move |_| {
                        if tx.send(MenuAction::Quit).is_err() {
                            tracing::warn!("tray action channel closed");
                        }
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
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
            if action == MenuAction::Quit {
                worker_quit.store(true, Ordering::Relaxed);
                break;
            }
            dispatch(action);
            let _ = worker_handle.update(|tray: &mut WallsSniTray| tray.refresh_state());
        }
    });

    let poll_handle = handle.clone();
    let mut watcher = StateWatcher::new().ok();
    let mut rotation = RotationLoop::new();
    while !quit.load(Ordering::Relaxed) {
        rotation.poll();
        if watcher.as_mut().is_some_and(|watcher| watcher.poll()) {
            let _ = poll_handle.update(|tray: &mut WallsSniTray| tray.refresh_state());
        }
        thread::sleep(Duration::from_millis(200));
    }

    handle.shutdown().wait();
    Ok(())
}

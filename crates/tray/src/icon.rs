use std::path::Path;

use image::imageops::FilterType;
use walls_core::paths::{expand_home, WallsPaths};
use walls_core::state::State;

const TRAY_SIZE: u32 = 32;

pub fn default_icon() -> anyhow::Result<tray_icon::Icon> {
    Ok(tray_icon::Icon::from_rgba(vec![80, 120, 200, 255], 1, 1)?)
}

pub fn icon_from_state() -> anyhow::Result<tray_icon::Icon> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let Some(current) = state.current else {
        return default_icon();
    };
    let path = expand_home(&current.composed_path);
    icon_from_path(&path).or_else(|_| default_icon())
}

pub fn tooltip_from_state() -> String {
    let Ok(paths) = WallsPaths::discover() else {
        return "walls".into();
    };
    let Ok(state) = State::load_or_default(&paths.state_file) else {
        return "walls".into();
    };
    if state.paused {
        "walls (paused)".into()
    } else {
        "walls".into()
    }
}

fn icon_from_path(path: &Path) -> anyhow::Result<tray_icon::Icon> {
    let img = image::open(path)?;
    let thumb = img.resize_to_fill(TRAY_SIZE, TRAY_SIZE, FilterType::Triangle);
    let rgba = thumb.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(tray_icon::Icon::from_rgba(rgba.into_raw(), w, h)?)
}

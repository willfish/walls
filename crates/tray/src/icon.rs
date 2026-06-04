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
    icon_from_current_path(&path)
}

pub fn tooltip_from_state() -> String {
    let Ok(paths) = WallsPaths::discover() else {
        return "walls".into();
    };
    let Ok(state) = State::load_or_default(&paths.state_file) else {
        return "walls".into();
    };
    tooltip_for_state(&state).into()
}

pub(crate) fn tooltip_for_state(state: &State) -> &'static str {
    if state.paused {
        "walls (paused)"
    } else {
        "walls"
    }
}

pub(crate) fn icon_from_current_path(path: &Path) -> anyhow::Result<tray_icon::Icon> {
    icon_from_path(path).or_else(|_| default_icon())
}

fn icon_from_path(path: &Path) -> anyhow::Result<tray_icon::Icon> {
    let img = image::open(path)?;
    let thumb = img.resize_to_fill(TRAY_SIZE, TRAY_SIZE, FilterType::Triangle);
    let rgba = thumb.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(tray_icon::Icon::from_rgba(rgba.into_raw(), w, h)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn tooltip_mentions_paused_state() {
        let paused = State {
            paused: true,
            ..State::default()
        };
        let running = State::default();

        assert_eq!(tooltip_for_state(&paused), "walls (paused)");
        assert_eq!(tooltip_for_state(&running), "walls");
    }

    #[test]
    fn icon_loads_thumbnail_from_current_composed_path() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(4, 4, Rgba([10u8, 20, 30, 255]));
        image.save(&image_path).unwrap();

        icon_from_current_path(&image_path).expect("thumbnail icon from composed wallpaper");
    }

    #[test]
    fn icon_falls_back_when_current_composed_path_cannot_load() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.png");

        icon_from_current_path(&missing).expect("default icon fallback");
    }
}

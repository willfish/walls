use std::path::Path;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};
use walls_core::paths::{expand_home, WallsPaths};
use walls_core::state::State;

const TRAY_SIZE: u32 = 32;
const TRAY_BORDER_PX: u32 = 2;
const BORDER_RGBA: [u8; 4] = [140, 190, 255, 255];
const FALLBACK_INNER_RGBA: [u8; 4] = [80, 120, 200, 255];

pub fn default_appindicator_icon() -> anyhow::Result<tray_icon::Icon> {
    let rgba = default_tray_rgba();
    Ok(tray_icon::Icon::from_rgba(rgba, TRAY_SIZE, TRAY_SIZE)?)
}

pub fn appindicator_icon_from_state() -> anyhow::Result<tray_icon::Icon> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let Some(current) = state.current else {
        return default_appindicator_icon();
    };
    let path = expand_home(&current.composed_path);
    appindicator_icon_from_path(&path)
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

pub fn default_ksni_icons() -> Vec<ksni::Icon> {
    let rgba = default_tray_rgba();
    vec![ksni::Icon {
        width: TRAY_SIZE as i32,
        height: TRAY_SIZE as i32,
        data: rgba_to_argb(&rgba),
    }]
}

pub fn ksni_icons_from_state() -> Vec<ksni::Icon> {
    ksni_icons_from_state_result().unwrap_or_default()
}

fn ksni_icons_from_state_result() -> anyhow::Result<Vec<ksni::Icon>> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let Some(current) = state.current else {
        return Ok(default_ksni_icons());
    };
    let path = expand_home(&current.composed_path);
    ksni_icons_from_path(&path).or_else(|_| Ok(default_ksni_icons()))
}

fn appindicator_icon_from_path(path: &Path) -> anyhow::Result<tray_icon::Icon> {
    rgba_icon_from_path(path).and_then(|rgba| {
        let (w, h) = (TRAY_SIZE, TRAY_SIZE);
        Ok(tray_icon::Icon::from_rgba(rgba, w, h)?)
    })
}

fn ksni_icons_from_path(path: &Path) -> anyhow::Result<Vec<ksni::Icon>> {
    let rgba = rgba_icon_from_path(path)?;
    Ok(vec![ksni::Icon {
        width: TRAY_SIZE as i32,
        height: TRAY_SIZE as i32,
        data: rgba_to_argb(&rgba),
    }])
}

fn inner_tray_size() -> u32 {
    TRAY_SIZE.saturating_sub(TRAY_BORDER_PX * 2).max(1)
}

fn default_tray_rgba() -> Vec<u8> {
    let inner = ImageBuffer::from_pixel(
        inner_tray_size(),
        inner_tray_size(),
        Rgba(FALLBACK_INNER_RGBA),
    );
    bordered_rgba(inner)
}

fn bordered_rgba(inner: RgbaImage) -> Vec<u8> {
    let mut canvas = ImageBuffer::from_pixel(TRAY_SIZE, TRAY_SIZE, Rgba(BORDER_RGBA));
    image::imageops::overlay(
        &mut canvas,
        &inner,
        i64::from(TRAY_BORDER_PX),
        i64::from(TRAY_BORDER_PX),
    );
    canvas.into_raw()
}

fn rgba_icon_from_path(path: &Path) -> anyhow::Result<Vec<u8>> {
    let img = image::open(path)?;
    let thumb = img.resize_to_fill(inner_tray_size(), inner_tray_size(), FilterType::Triangle);
    Ok(bordered_rgba(thumb.to_rgba8()))
}

fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
        .collect()
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
    fn appindicator_icon_loads_thumbnail_from_current_composed_path() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(4, 4, Rgba([10u8, 20, 30, 255]));
        image.save(&image_path).unwrap();

        appindicator_icon_from_path(&image_path).expect("thumbnail icon from composed wallpaper");
    }

    #[test]
    fn appindicator_icon_falls_back_when_current_composed_path_cannot_load() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.png");

        appindicator_icon_from_path(&missing)
            .or_else(|_| default_appindicator_icon())
            .expect("default icon fallback");
    }

    #[test]
    fn ksni_icon_loads_from_wallpaper_path() {
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(4, 4, Rgba([10u8, 20, 30, 255]));
        image.save(&image_path).unwrap();

        let icons = ksni_icons_from_path(&image_path).expect("ksni icons");
        assert_eq!(icons[0].width, TRAY_SIZE as i32);
        assert_eq!(icons[0].data.len(), (TRAY_SIZE * TRAY_SIZE * 4) as usize);
    }

    #[test]
    fn tray_icon_has_light_blue_border() {
        let rgba = default_tray_rgba();
        assert_border_pixel(&rgba, 0, 0);
        assert_border_pixel(&rgba, TRAY_SIZE - 1, 0);
        assert_border_pixel(&rgba, 0, TRAY_SIZE - 1);

        let inner = TRAY_BORDER_PX + inner_tray_size() / 2;
        let center = pixel_rgba(&rgba, inner, inner);
        assert_eq!(center, FALLBACK_INNER_RGBA);
    }

    fn pixel_rgba(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * TRAY_SIZE + x) * 4) as usize;
        rgba[idx..idx + 4].try_into().unwrap()
    }

    fn assert_border_pixel(rgba: &[u8], x: u32, y: u32) {
        assert_eq!(pixel_rgba(rgba, x, y), BORDER_RGBA);
    }
}

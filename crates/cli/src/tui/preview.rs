use std::path::Path;

use image::ImageReader;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

pub struct ImagePreview {
    picker: Option<Picker>,
    cached: Option<CachedPreview>,
    status: String,
}

struct CachedPreview {
    path: String,
    size: Size,
    protocol: Protocol,
}

impl ImagePreview {
    pub fn detect() -> Self {
        if preview_disabled() {
            return Self {
                picker: None,
                cached: None,
                status: "preview disabled; showing metadata".into(),
            };
        }

        if !supported_terminal_hint() {
            return Self {
                picker: None,
                cached: None,
                status: "preview unsupported; showing metadata".into(),
            };
        }

        match Picker::from_query_stdio() {
            Ok(picker)
                if matches!(
                    picker.protocol_type(),
                    ProtocolType::Kitty | ProtocolType::Iterm2
                ) =>
            {
                let status = format!("preview: {:?}", picker.protocol_type()).to_lowercase();
                Self {
                    picker: Some(picker),
                    cached: None,
                    status,
                }
            }
            Ok(picker) => Self {
                picker: None,
                cached: None,
                status: format!(
                    "preview unsupported ({:?}); showing metadata",
                    picker.protocol_type()
                )
                .to_lowercase(),
            },
            Err(err) => Self {
                picker: None,
                cached: None,
                status: format!("preview unavailable: {err}; showing metadata"),
            },
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, path: Option<&str>) {
        let block = Block::default().borders(Borders::ALL).title("preview");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(path) = path else {
            self.render_fallback(f, inner, "(no current wallpaper)");
            return;
        };
        if self.picker.is_none() {
            let status = self.status.clone();
            self.render_fallback(f, inner, &status);
            return;
        }
        if inner.width < 8 || inner.height < 4 {
            self.render_fallback(f, inner, "preview needs more space");
            return;
        }

        let size = Size::new(inner.width, inner.height);
        if !self.cache_matches(path, size) {
            if let Err(err) = self.load(path, size) {
                self.cached = None;
                self.status = format!("preview failed: {err}; showing metadata");
                let status = self.status.clone();
                self.render_fallback(f, inner, &status);
                return;
            }
        }

        if let Some(cached) = &self.cached {
            f.render_widget(Image::new(&cached.protocol).allow_clipping(true), inner);
        } else {
            self.render_fallback(f, inner, "preview unavailable; showing metadata");
        }
    }

    fn cache_matches(&self, path: &str, size: Size) -> bool {
        self.cached
            .as_ref()
            .is_some_and(|cached| cached.path == path && cached.size == size)
    }

    fn load(&mut self, path: &str, size: Size) -> anyhow::Result<()> {
        let image = ImageReader::open(Path::new(path))?.decode()?;
        let protocol = self
            .picker
            .as_ref()
            .expect("checked by caller")
            .new_protocol(image, size, Resize::Fit(None))?;
        self.cached = Some(CachedPreview {
            path: path.to_string(),
            size,
            protocol,
        });
        Ok(())
    }

    fn render_fallback(&self, f: &mut Frame, area: Rect, message: &str) {
        f.render_widget(Paragraph::new(message.to_string()), area);
    }
}

fn preview_disabled() -> bool {
    std::env::var("WALLS_TUI_PREVIEW").is_ok_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

fn supported_terminal_hint() -> bool {
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();

    term.contains("ghostty")
        || term.contains("kitty")
        || term_program.contains("ghostty")
        || term_program.contains("kitty")
        || std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some()
        || std::env::var_os("KITTY_WINDOW_ID").is_some()
        || term_program.contains("iterm")
        || std::env::var_os("ITERM_SESSION_ID").is_some()
}

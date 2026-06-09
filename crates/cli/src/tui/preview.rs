use std::path::Path;

use image::ImageReader;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

use super::style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewCapability {
    Disabled,
    ProbeProtocol,
    Unsupported,
}

#[derive(Debug, Default, Clone)]
struct TerminalHints {
    term: String,
    term_program: String,
    ghostty_resources: bool,
    kitty_window: bool,
    iterm_session: bool,
}

pub struct ImagePreview {
    picker: Option<Picker>,
    cached: Option<CachedPreview>,
    status: PreviewFallback,
}

#[derive(Debug, Clone)]
struct PreviewFallback {
    kind: style::StateKind,
    message: String,
}

struct CachedPreview {
    path: String,
    size: Size,
    protocol: Protocol,
}

impl ImagePreview {
    pub fn detect() -> Self {
        match PreviewCapability::from_env() {
            PreviewCapability::Disabled => {
                return Self {
                    picker: None,
                    cached: None,
                    status: PreviewFallback::new(
                        style::StateKind::Disabled,
                        "preview disabled; showing metadata",
                    ),
                };
            }
            PreviewCapability::Unsupported => {
                return Self {
                    picker: None,
                    cached: None,
                    status: PreviewFallback::new(
                        style::StateKind::Unavailable,
                        "preview unsupported; showing metadata",
                    ),
                };
            }
            PreviewCapability::ProbeProtocol => {}
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
                    status: PreviewFallback::new(style::StateKind::Loading, status),
                }
            }
            Ok(picker) => Self {
                picker: None,
                cached: None,
                status: PreviewFallback::new(
                    style::StateKind::Unavailable,
                    format!(
                        "preview unsupported ({:?}); showing metadata",
                        picker.protocol_type()
                    )
                    .to_lowercase(),
                ),
            },
            Err(err) => Self {
                picker: None,
                cached: None,
                status: PreviewFallback::new(
                    style::StateKind::Unavailable,
                    format!("preview unavailable: {err}; showing metadata"),
                ),
            },
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, path: Option<&str>, theme: style::Theme) {
        let block = theme.content_block("preview");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(path) = path else {
            self.render_fallback(
                f,
                inner,
                style::StateKind::Empty,
                "no current wallpaper",
                theme,
            );
            return;
        };
        if self.picker.is_none() {
            let status = self.status.clone();
            self.render_fallback(f, inner, status.kind, &status.message, theme);
            return;
        }
        if inner.width < 8 || inner.height < 4 {
            self.render_fallback(
                f,
                inner,
                style::StateKind::Unavailable,
                "preview needs more space",
                theme,
            );
            return;
        }

        let size = Size::new(inner.width, inner.height);
        if !self.cache_matches(path, size) {
            if let Err(err) = self.load(path, size) {
                self.cached = None;
                self.status = PreviewFallback::new(
                    style::StateKind::ValidationError,
                    format!("preview failed: {err}; showing metadata"),
                );
                let status = self.status.clone();
                self.render_fallback(f, inner, status.kind, &status.message, theme);
                return;
            }
        }

        if let Some(cached) = &self.cached {
            f.render_widget(Image::new(&cached.protocol).allow_clipping(true), inner);
        } else {
            self.render_fallback(
                f,
                inner,
                style::StateKind::Unavailable,
                "preview unavailable; showing metadata",
                theme,
            );
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

    fn render_fallback(
        &self,
        f: &mut Frame,
        area: Rect,
        kind: style::StateKind,
        message: &str,
        theme: style::Theme,
    ) {
        f.render_widget(
            Paragraph::new(style::state_line(kind, message.to_string(), theme)),
            area,
        );
    }
}

impl PreviewFallback {
    fn new(kind: style::StateKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl PreviewCapability {
    fn from_env() -> Self {
        let disabled = std::env::var("WALLS_TUI_PREVIEW")
            .ok()
            .as_deref()
            .is_some_and(preview_disabled_value);
        Self::from_hints(disabled, &TerminalHints::from_env())
    }

    fn from_hints(disabled: bool, hints: &TerminalHints) -> Self {
        if disabled {
            return Self::Disabled;
        }

        if hints.supports_known_image_protocol() {
            Self::ProbeProtocol
        } else {
            Self::Unsupported
        }
    }
}

impl TerminalHints {
    fn from_env() -> Self {
        Self {
            term: std::env::var("TERM").unwrap_or_default(),
            term_program: std::env::var("TERM_PROGRAM").unwrap_or_default(),
            ghostty_resources: std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some(),
            kitty_window: std::env::var_os("KITTY_WINDOW_ID").is_some(),
            iterm_session: std::env::var_os("ITERM_SESSION_ID").is_some(),
        }
    }

    fn supports_known_image_protocol(&self) -> bool {
        let term = self.term.to_ascii_lowercase();
        let term_program = self.term_program.to_ascii_lowercase();

        term.contains("ghostty")
            || term.contains("kitty")
            || term_program.contains("ghostty")
            || term_program.contains("kitty")
            || self.ghostty_resources
            || self.kitty_window
            || term_program.contains("iterm")
            || self.iterm_session
    }
}

fn preview_disabled_value(value: &str) -> bool {
    {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off" | "never" | "metadata"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{preview_disabled_value, PreviewCapability, TerminalHints};

    #[test]
    fn preview_disable_values_force_metadata_only_mode() {
        for value in ["0", "false", "no", "off", "never", "metadata"] {
            assert!(preview_disabled_value(value), "{value}");
        }
        assert!(!preview_disabled_value("1"));
        assert!(!preview_disabled_value("true"));
    }

    #[test]
    fn ghostty_and_kitty_hints_probe_for_image_protocol() {
        let ghostty = TerminalHints {
            term_program: "ghostty".into(),
            ..TerminalHints::default()
        };
        let ghostty_resources = TerminalHints {
            ghostty_resources: true,
            ..TerminalHints::default()
        };
        let kitty = TerminalHints {
            kitty_window: true,
            ..TerminalHints::default()
        };

        assert_eq!(
            PreviewCapability::from_hints(false, &ghostty),
            PreviewCapability::ProbeProtocol
        );
        assert_eq!(
            PreviewCapability::from_hints(false, &ghostty_resources),
            PreviewCapability::ProbeProtocol
        );
        assert_eq!(
            PreviewCapability::from_hints(false, &kitty),
            PreviewCapability::ProbeProtocol
        );
    }

    #[test]
    fn iterm_hints_probe_and_unknown_terminals_stay_metadata_only() {
        let iterm = TerminalHints {
            term_program: "iTerm.app".into(),
            ..TerminalHints::default()
        };
        let unknown = TerminalHints {
            term: "xterm-256color".into(),
            ..TerminalHints::default()
        };

        assert_eq!(
            PreviewCapability::from_hints(false, &iterm),
            PreviewCapability::ProbeProtocol
        );
        assert_eq!(
            PreviewCapability::from_hints(false, &unknown),
            PreviewCapability::Unsupported
        );
        assert_eq!(
            PreviewCapability::from_hints(true, &iterm),
            PreviewCapability::Disabled
        );
    }
}

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Neutral,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    color_mode: ColorMode,
}

impl ColorMode {
    pub fn from_env() -> Self {
        Self::parse(std::env::var("WALLS_TUI_COLOR").ok().as_deref())
    }

    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("0" | "false" | "no" | "off" | "never" | "none" | "plain") => Self::Never,
            _ => Self::Auto,
        }
    }
}

impl Theme {
    pub fn new(color_mode: ColorMode) -> Self {
        Self { color_mode }
    }

    /// Frame persistent chrome such as the tabs header or footer.
    pub fn chrome_block<'a>(self, title: &'a str) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(self.border())
            .title_style(self.accent())
    }

    /// Frame the active tab body, preview pane, or focused tool surface.
    pub fn content_block<'a>(self, title: &'a str) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(self.border())
            .title_style(self.heading())
    }

    /// Ordinary readable body text with no extra hierarchy.
    pub fn normal(self) -> Style {
        Style::default()
    }

    /// Secondary metadata, unavailable text, hints, and quiet separators.
    pub fn muted(self) -> Style {
        match self.color_mode {
            ColorMode::Auto => Style::default().fg(Color::DarkGray),
            ColorMode::Never => Style::default().add_modifier(Modifier::DIM),
        }
    }

    /// Primary hierarchy cue for titles and important labels.
    pub fn accent(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD);
        match self.color_mode {
            ColorMode::Auto => style.fg(Color::Cyan),
            ColorMode::Never => style,
        }
    }

    /// Colour-neutral strong label for compact headings and enabled names.
    pub fn heading(self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// Selected list row, tab, or command target.
    pub fn selected(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED);
        match self.color_mode {
            ColorMode::Auto => style.fg(Color::Black).bg(Color::Cyan),
            ColorMode::Never => style,
        }
    }

    /// High-contrast row background for the focused edit-form field (labels must stay readable).
    pub fn edit_focus_row(self) -> Style {
        match self.color_mode {
            ColorMode::Auto => Style::default().fg(Color::Black).bg(Color::Cyan),
            ColorMode::Never => Style::default().add_modifier(Modifier::REVERSED),
        }
    }

    /// Label segment inside the active edit-form row.
    pub fn edit_focus_label(self) -> Style {
        let row = self.edit_focus_row();
        row.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// Value segment inside the active edit-form row.
    pub fn edit_focus_value(self) -> Style {
        self.edit_focus_row().add_modifier(Modifier::BOLD)
    }

    /// Default block border treatment.
    pub fn border(self) -> Style {
        match self.color_mode {
            ColorMode::Auto => Style::default().fg(Color::DarkGray),
            ColorMode::Never => Style::default(),
        }
    }

    /// Compact keyboard affordance in chrome or status areas.
    pub fn key_hint(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD);
        match self.color_mode {
            ColorMode::Auto => style.fg(Color::Yellow),
            ColorMode::Never => style,
        }
    }

    /// Result or validation state. Text must still name the state in no-colour mode.
    pub fn status(self, kind: StatusKind) -> Style {
        match (self.color_mode, kind) {
            (_, StatusKind::Neutral) => self.muted(),
            (ColorMode::Auto, StatusKind::Success) => Style::default().fg(Color::Green),
            (ColorMode::Auto, StatusKind::Warning) => Style::default().fg(Color::Yellow),
            (ColorMode::Auto, StatusKind::Error) => {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            }
            (ColorMode::Never, StatusKind::Success) => Style::default(),
            (ColorMode::Never, StatusKind::Warning) => {
                Style::default().add_modifier(Modifier::BOLD)
            }
            (ColorMode::Never, StatusKind::Error) => {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            }
        }
    }
}

pub fn status_kind(message: &str) -> StatusKind {
    let lower = message.to_ascii_lowercase();
    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("unavailable")
        || lower.contains("unsupported")
    {
        StatusKind::Error
    } else if lower.contains("missing")
        || lower.contains("disabled")
        || lower.contains("needs more space")
    {
        StatusKind::Warning
    } else if lower.contains("applied")
        || lower.contains("favorited")
        || lower.contains("next:")
        || lower.contains("prev:")
        || lower.contains("search:")
    {
        StatusKind::Success
    } else {
        StatusKind::Neutral
    }
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::{Color, Modifier, Style};

    use super::{ColorMode, StatusKind, Theme};

    #[test]
    fn color_mode_parses_no_colour_aliases() {
        for value in ["0", "false", "no", "off", "never", "none", "plain"] {
            assert_eq!(ColorMode::parse(Some(value)), ColorMode::Never);
        }
        assert_eq!(ColorMode::parse(Some("auto")), ColorMode::Auto);
        assert_eq!(ColorMode::parse(None), ColorMode::Auto);
    }

    #[test]
    fn no_colour_theme_uses_modifiers_without_foreground_colours() {
        let theme = Theme::new(ColorMode::Never);

        assert_eq!(theme.border().fg, None);
        assert_eq!(theme.key_hint().fg, None);
        assert!(theme.selected().add_modifier.contains(Modifier::REVERSED));
        assert_eq!(
            theme.status(StatusKind::Error),
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        );
    }

    #[test]
    fn default_theme_uses_redundant_semantic_colours() {
        let theme = Theme::new(ColorMode::Auto);

        assert_eq!(theme.status(StatusKind::Success).fg, Some(Color::Green));
        assert_eq!(theme.status(StatusKind::Warning).fg, Some(Color::Yellow));
        assert_eq!(theme.status(StatusKind::Error).fg, Some(Color::Red));
        assert!(theme
            .status(StatusKind::Error)
            .add_modifier
            .contains(Modifier::BOLD));
    }
}

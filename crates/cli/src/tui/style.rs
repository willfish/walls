use ratatui::prelude::*;
use ratatui::text::{Line, Span};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Empty,
    Disabled,
    Unavailable,
    MissingConfig,
    ValidationWarning,
    ValidationError,
    Loading,
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
        Style::default().add_modifier(Modifier::DIM)
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

    /// Active/enabled state, distinct from successful operation feedback.
    pub fn active_state(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD);
        match self.color_mode {
            ColorMode::Auto => style.fg(Color::Cyan),
            ColorMode::Never => style,
        }
    }

    /// Inactive/off state, distinct from failed operation feedback.
    pub fn inactive_state(self) -> Style {
        self.muted()
    }

    /// Boolean true value in config forms. This is state, not success.
    pub fn boolean_true(self) -> Style {
        self.active_state()
    }

    /// Boolean false value in config forms. This is state, not error.
    pub fn boolean_false(self) -> Style {
        self.inactive_state()
    }

    /// Unavailable-but-actionable state. Reserve error styling for failures and validation errors.
    pub fn unavailable(self) -> Style {
        self.status(StatusKind::Warning)
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

    pub fn state(self, kind: StateKind) -> Style {
        self.status(kind.status_kind())
    }
}

impl StateKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
            Self::MissingConfig => "missing",
            Self::ValidationWarning => "warning",
            Self::ValidationError => "error",
            Self::Loading => "loading",
        }
    }

    fn status_kind(self) -> StatusKind {
        match self {
            Self::Empty | Self::Loading => StatusKind::Neutral,
            Self::Disabled | Self::Unavailable | Self::MissingConfig | Self::ValidationWarning => {
                StatusKind::Warning
            }
            Self::ValidationError => StatusKind::Error,
        }
    }
}

pub fn state_text(kind: StateKind, message: impl AsRef<str>) -> String {
    format!("[{}] {}", kind.label(), message.as_ref())
}

pub fn state_parts(text: &str) -> Option<(StateKind, &str)> {
    let trimmed = text.trim_start();
    let (label, rest) = trimmed.strip_prefix('[')?.split_once("] ")?;
    let kind = match label {
        "empty" => StateKind::Empty,
        "disabled" => StateKind::Disabled,
        "unavailable" => StateKind::Unavailable,
        "missing" => StateKind::MissingConfig,
        "warning" => StateKind::ValidationWarning,
        "error" => StateKind::ValidationError,
        "loading" => StateKind::Loading,
        _ => return None,
    };
    Some((kind, rest))
}

pub fn state_line(kind: StateKind, message: impl Into<String>, theme: Theme) -> Line<'static> {
    let message = message.into();
    Line::from(vec![
        Span::styled(format!("[{}] ", kind.label()), theme.state(kind)),
        Span::styled(message, theme.state(kind)),
    ])
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::{Color, Modifier, Style};

    use super::{state_text, ColorMode, StateKind, StatusKind, Theme};

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
        assert_eq!(theme.active_state().fg, None);
        assert_eq!(theme.inactive_state().fg, None);
        assert_eq!(theme.boolean_true().fg, None);
        assert_eq!(theme.boolean_false().fg, None);
        assert_eq!(theme.unavailable().fg, None);
        assert!(theme.selected().add_modifier.contains(Modifier::REVERSED));
        assert!(theme.active_state().add_modifier.contains(Modifier::BOLD));
        assert!(theme.inactive_state().add_modifier.contains(Modifier::DIM));
        assert!(theme.boolean_true().add_modifier.contains(Modifier::BOLD));
        assert!(theme.boolean_false().add_modifier.contains(Modifier::DIM));
        assert!(theme.unavailable().add_modifier.contains(Modifier::BOLD));
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
        assert_eq!(theme.active_state().fg, Some(Color::Cyan));
        assert_eq!(theme.inactive_state().fg, None);
        assert_eq!(theme.boolean_true().fg, Some(Color::Cyan));
        assert_eq!(theme.boolean_false().fg, None);
        assert_eq!(theme.unavailable().fg, Some(Color::Yellow));
        assert!(theme.muted().add_modifier.contains(Modifier::DIM));
        assert_ne!(theme.boolean_false().fg, theme.status(StatusKind::Error).fg);
        assert_ne!(
            theme.boolean_true().fg,
            theme.status(StatusKind::Success).fg
        );
        assert!(theme
            .status(StatusKind::Error)
            .add_modifier
            .contains(Modifier::BOLD));
    }

    #[test]
    fn state_text_uses_no_colour_labels() {
        assert_eq!(state_text(StateKind::Empty, "no logs"), "[empty] no logs");
        assert_eq!(
            state_text(StateKind::MissingConfig, "path not set"),
            "[missing] path not set"
        );
        assert_eq!(
            state_text(StateKind::ValidationWarning, "API key missing"),
            "[warning] API key missing"
        );
    }

    #[test]
    fn state_roles_have_no_colour_mode_fallbacks() {
        let theme = Theme::new(ColorMode::Never);

        assert!(theme
            .state(StateKind::Empty)
            .add_modifier
            .contains(Modifier::DIM));
        assert!(theme
            .state(StateKind::ValidationWarning)
            .add_modifier
            .contains(Modifier::BOLD));
        assert!(theme
            .state(StateKind::ValidationError)
            .add_modifier
            .contains(Modifier::REVERSED));
    }
}

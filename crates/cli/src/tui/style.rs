use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};
use walls_core::config::TuiTheme;

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
    preset: TuiTheme,
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    accent: Color,
    selected_fg: Color,
    selected_bg: Color,
    edit_fg: Color,
    edit_bg: Color,
    border: Color,
    key_hint: Color,
    active: Color,
    success: Color,
    warning: Color,
    error: Color,
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
        Self::with_preset(color_mode, TuiTheme::Auto)
    }

    pub fn with_preset(color_mode: ColorMode, preset: TuiTheme) -> Self {
        Self { color_mode, preset }
    }

    fn colors_enabled(self) -> bool {
        self.color_mode == ColorMode::Auto && self.preset != TuiTheme::Plain
    }

    fn palette(self) -> Palette {
        match self.preset {
            TuiTheme::Auto | TuiTheme::Plain => Palette {
                accent: Color::Cyan,
                selected_fg: Color::Black,
                selected_bg: Color::Cyan,
                edit_fg: Color::Black,
                edit_bg: Color::Cyan,
                border: Color::DarkGray,
                key_hint: Color::Yellow,
                active: Color::Cyan,
                success: Color::Green,
                warning: Color::Yellow,
                error: Color::Red,
            },
            TuiTheme::Gruvbox => Palette {
                accent: Color::Rgb(131, 165, 152),
                selected_fg: Color::Rgb(250, 189, 47),
                selected_bg: Color::Rgb(60, 56, 54),
                edit_fg: Color::Rgb(235, 219, 178),
                edit_bg: Color::Rgb(80, 73, 69),
                border: Color::Rgb(102, 92, 84),
                key_hint: Color::Rgb(254, 128, 25),
                active: Color::Rgb(69, 133, 136),
                success: Color::Rgb(184, 187, 38),
                warning: Color::Rgb(250, 189, 47),
                error: Color::Rgb(251, 73, 52),
            },
            TuiTheme::RosePine => Palette {
                accent: Color::Rgb(235, 188, 186),
                selected_fg: Color::Rgb(235, 188, 186),
                selected_bg: Color::Rgb(64, 61, 82),
                edit_fg: Color::Rgb(224, 222, 244),
                edit_bg: Color::Rgb(38, 35, 58),
                border: Color::Rgb(82, 79, 103),
                key_hint: Color::Rgb(246, 193, 119),
                active: Color::Rgb(156, 207, 216),
                success: Color::Rgb(49, 116, 143),
                warning: Color::Rgb(246, 193, 119),
                error: Color::Rgb(235, 111, 146),
            },
            TuiTheme::Nord => Palette {
                accent: Color::Rgb(136, 192, 208),
                selected_fg: Color::Rgb(136, 192, 208),
                selected_bg: Color::Rgb(67, 76, 94),
                edit_fg: Color::Rgb(229, 233, 240),
                edit_bg: Color::Rgb(59, 66, 82),
                border: Color::Rgb(76, 86, 106),
                key_hint: Color::Rgb(235, 203, 139),
                active: Color::Rgb(94, 129, 172),
                success: Color::Rgb(163, 190, 140),
                warning: Color::Rgb(235, 203, 139),
                error: Color::Rgb(191, 97, 106),
            },
            TuiTheme::Catppuccin => Palette {
                accent: Color::Rgb(137, 180, 250),
                selected_fg: Color::Rgb(203, 166, 247),
                selected_bg: Color::Rgb(69, 71, 90),
                edit_fg: Color::Rgb(205, 214, 244),
                edit_bg: Color::Rgb(49, 50, 68),
                border: Color::Rgb(108, 112, 134),
                key_hint: Color::Rgb(250, 179, 135),
                active: Color::Rgb(116, 199, 236),
                success: Color::Rgb(166, 227, 161),
                warning: Color::Rgb(249, 226, 175),
                error: Color::Rgb(243, 139, 168),
            },
            TuiTheme::TokyoNight => Palette {
                accent: Color::Rgb(125, 207, 255),
                selected_fg: Color::Rgb(122, 162, 247),
                selected_bg: Color::Rgb(41, 46, 66),
                edit_fg: Color::Rgb(192, 202, 245),
                edit_bg: Color::Rgb(36, 40, 59),
                border: Color::Rgb(65, 72, 104),
                key_hint: Color::Rgb(255, 158, 100),
                active: Color::Rgb(42, 195, 222),
                success: Color::Rgb(158, 206, 106),
                warning: Color::Rgb(224, 175, 104),
                error: Color::Rgb(247, 118, 142),
            },
            TuiTheme::Dracula => Palette {
                accent: Color::Rgb(189, 147, 249),
                selected_fg: Color::Rgb(248, 248, 242),
                selected_bg: Color::Rgb(68, 71, 90),
                edit_fg: Color::Rgb(248, 248, 242),
                edit_bg: Color::Rgb(68, 71, 90),
                border: Color::Rgb(98, 114, 164),
                key_hint: Color::Rgb(255, 184, 108),
                active: Color::Rgb(139, 233, 253),
                success: Color::Rgb(80, 250, 123),
                warning: Color::Rgb(241, 250, 140),
                error: Color::Rgb(255, 85, 85),
            },
            TuiTheme::SolarizedDark => Palette {
                accent: Color::Rgb(38, 139, 210),
                selected_fg: Color::Rgb(38, 139, 210),
                selected_bg: Color::Rgb(7, 54, 66),
                edit_fg: Color::Rgb(131, 148, 150),
                edit_bg: Color::Rgb(7, 54, 66),
                border: Color::Rgb(88, 110, 117),
                key_hint: Color::Rgb(181, 137, 0),
                active: Color::Rgb(42, 161, 152),
                success: Color::Rgb(133, 153, 0),
                warning: Color::Rgb(203, 75, 22),
                error: Color::Rgb(220, 50, 47),
            },
            TuiTheme::SolarizedLight => Palette {
                accent: Color::Rgb(38, 139, 210),
                selected_fg: Color::Rgb(38, 139, 210),
                selected_bg: Color::Rgb(238, 232, 213),
                edit_fg: Color::Rgb(101, 123, 131),
                edit_bg: Color::Rgb(238, 232, 213),
                border: Color::Rgb(147, 161, 161),
                key_hint: Color::Rgb(181, 137, 0),
                active: Color::Rgb(42, 161, 152),
                success: Color::Rgb(133, 153, 0),
                warning: Color::Rgb(203, 75, 22),
                error: Color::Rgb(220, 50, 47),
            },
            TuiTheme::Everforest => Palette {
                accent: Color::Rgb(127, 187, 179),
                selected_fg: Color::Rgb(127, 187, 179),
                selected_bg: Color::Rgb(52, 63, 68),
                edit_fg: Color::Rgb(211, 198, 170),
                edit_bg: Color::Rgb(79, 88, 94),
                border: Color::Rgb(133, 146, 137),
                key_hint: Color::Rgb(230, 152, 117),
                active: Color::Rgb(131, 192, 146),
                success: Color::Rgb(167, 192, 128),
                warning: Color::Rgb(219, 188, 127),
                error: Color::Rgb(230, 126, 128),
            },
            TuiTheme::Kanagawa => Palette {
                accent: Color::Rgb(149, 127, 184),
                selected_fg: Color::Rgb(126, 156, 216),
                selected_bg: Color::Rgb(34, 50, 73),
                edit_fg: Color::Rgb(220, 215, 186),
                edit_bg: Color::Rgb(42, 42, 55),
                border: Color::Rgb(84, 84, 109),
                key_hint: Color::Rgb(230, 195, 132),
                active: Color::Rgb(126, 156, 216),
                success: Color::Rgb(152, 187, 108),
                warning: Color::Rgb(255, 158, 59),
                error: Color::Rgb(232, 36, 36),
            },
            TuiTheme::Monokai => Palette {
                accent: Color::Rgb(102, 217, 239),
                selected_fg: Color::Rgb(230, 219, 116),
                selected_bg: Color::Rgb(73, 72, 62),
                edit_fg: Color::Rgb(248, 248, 242),
                edit_bg: Color::Rgb(73, 72, 62),
                border: Color::Rgb(117, 113, 94),
                key_hint: Color::Rgb(253, 151, 31),
                active: Color::Rgb(102, 217, 239),
                success: Color::Rgb(166, 226, 46),
                warning: Color::Rgb(230, 219, 116),
                error: Color::Rgb(249, 38, 114),
            },
            TuiTheme::OneDark => Palette {
                accent: Color::Rgb(97, 175, 239),
                selected_fg: Color::Rgb(97, 175, 239),
                selected_bg: Color::Rgb(60, 64, 72),
                edit_fg: Color::Rgb(171, 178, 191),
                edit_bg: Color::Rgb(44, 49, 58),
                border: Color::Rgb(92, 99, 112),
                key_hint: Color::Rgb(209, 154, 102),
                active: Color::Rgb(86, 182, 194),
                success: Color::Rgb(152, 195, 121),
                warning: Color::Rgb(229, 192, 123),
                error: Color::Rgb(224, 108, 117),
            },
            TuiTheme::AyuDark => Palette {
                accent: Color::Rgb(79, 191, 255),
                selected_fg: Color::Rgb(79, 191, 255),
                selected_bg: Color::Rgb(27, 31, 41),
                edit_fg: Color::Rgb(191, 189, 182),
                edit_bg: Color::Rgb(27, 31, 41),
                border: Color::Rgb(27, 31, 41),
                key_hint: Color::Rgb(255, 180, 84),
                active: Color::Rgb(79, 191, 255),
                success: Color::Rgb(112, 191, 86),
                warning: Color::Rgb(230, 180, 80),
                error: Color::Rgb(240, 107, 115),
            },
            TuiTheme::GithubDark => Palette {
                accent: Color::Rgb(88, 166, 255),
                selected_fg: Color::Rgb(88, 166, 255),
                selected_bg: Color::Rgb(22, 27, 34),
                edit_fg: Color::Rgb(230, 237, 243),
                edit_bg: Color::Rgb(22, 27, 34),
                border: Color::Rgb(48, 54, 61),
                key_hint: Color::Rgb(210, 153, 34),
                active: Color::Rgb(88, 166, 255),
                success: Color::Rgb(63, 185, 80),
                warning: Color::Rgb(210, 153, 34),
                error: Color::Rgb(248, 81, 73),
            },
        }
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
        if self.colors_enabled() {
            style.fg(self.palette().accent)
        } else {
            style
        }
    }

    /// Colour-neutral strong label for compact headings and enabled names.
    pub fn heading(self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// Active/enabled state, distinct from successful operation feedback.
    pub fn active_state(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD);
        if self.colors_enabled() {
            style.fg(self.palette().active)
        } else {
            style
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
        if self.colors_enabled() {
            let palette = self.palette();
            style.fg(palette.selected_fg).bg(palette.selected_bg)
        } else {
            style
        }
    }

    /// High-contrast row background for the focused edit-form field (labels must stay readable).
    pub fn edit_focus_row(self) -> Style {
        if self.colors_enabled() {
            let palette = self.palette();
            Style::default().fg(palette.edit_fg).bg(palette.edit_bg)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
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
        if self.colors_enabled() {
            Style::default().fg(self.palette().border)
        } else {
            Style::default()
        }
    }

    /// Compact keyboard affordance in chrome or status areas.
    pub fn key_hint(self) -> Style {
        let style = Style::default().add_modifier(Modifier::BOLD);
        if self.colors_enabled() {
            style.fg(self.palette().key_hint)
        } else {
            style
        }
    }

    /// Result or validation state. Text must still name the state in no-colour mode.
    pub fn status(self, kind: StatusKind) -> Style {
        if kind == StatusKind::Neutral {
            return self.muted();
        }
        if !self.colors_enabled() {
            return match kind {
                StatusKind::Neutral => self.muted(),
                StatusKind::Success => Style::default(),
                StatusKind::Warning => Style::default().add_modifier(Modifier::BOLD),
                StatusKind::Error => {
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                }
            };
        }

        let palette = self.palette();
        match kind {
            StatusKind::Neutral => self.muted(),
            StatusKind::Success => Style::default().fg(palette.success),
            StatusKind::Warning => Style::default().fg(palette.warning),
            StatusKind::Error => Style::default()
                .fg(palette.error)
                .add_modifier(Modifier::BOLD),
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
    use walls_core::config::TuiTheme;

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
    fn classic_theme_presets_keep_semantic_roles_distinct() {
        for preset in [
            TuiTheme::Gruvbox,
            TuiTheme::RosePine,
            TuiTheme::Nord,
            TuiTheme::Catppuccin,
            TuiTheme::TokyoNight,
            TuiTheme::Dracula,
            TuiTheme::SolarizedDark,
            TuiTheme::SolarizedLight,
            TuiTheme::Everforest,
            TuiTheme::Kanagawa,
            TuiTheme::Monokai,
            TuiTheme::OneDark,
            TuiTheme::AyuDark,
            TuiTheme::GithubDark,
        ] {
            let theme = Theme::with_preset(ColorMode::Auto, preset);

            assert_ne!(theme.selected().bg, None, "{preset:?} selected bg");
            assert_ne!(theme.selected().fg, theme.selected().bg, "{preset:?}");
            assert_ne!(theme.key_hint().fg, theme.accent().fg, "{preset:?}");
            assert_ne!(theme.key_hint().fg, theme.active_state().fg, "{preset:?}");
            assert_ne!(
                theme.status(StatusKind::Warning).fg,
                theme.inactive_state().fg,
                "{preset:?}"
            );
            assert_ne!(
                theme.status(StatusKind::Error).fg,
                theme.status(StatusKind::Warning).fg,
                "{preset:?}"
            );
            assert_ne!(
                theme.status(StatusKind::Success).fg,
                theme.active_state().fg,
                "{preset:?}"
            );
        }
    }

    #[test]
    fn theme_presets_keep_canonical_anchor_colours() {
        for (preset, accent, key_hint, success, warning, error) in [
            (
                TuiTheme::RosePine,
                Color::Rgb(235, 188, 186),
                Color::Rgb(246, 193, 119),
                Color::Rgb(49, 116, 143),
                Color::Rgb(246, 193, 119),
                Color::Rgb(235, 111, 146),
            ),
            (
                TuiTheme::Dracula,
                Color::Rgb(189, 147, 249),
                Color::Rgb(255, 184, 108),
                Color::Rgb(80, 250, 123),
                Color::Rgb(241, 250, 140),
                Color::Rgb(255, 85, 85),
            ),
            (
                TuiTheme::SolarizedDark,
                Color::Rgb(38, 139, 210),
                Color::Rgb(181, 137, 0),
                Color::Rgb(133, 153, 0),
                Color::Rgb(203, 75, 22),
                Color::Rgb(220, 50, 47),
            ),
            (
                TuiTheme::SolarizedLight,
                Color::Rgb(38, 139, 210),
                Color::Rgb(181, 137, 0),
                Color::Rgb(133, 153, 0),
                Color::Rgb(203, 75, 22),
                Color::Rgb(220, 50, 47),
            ),
            (
                TuiTheme::Everforest,
                Color::Rgb(127, 187, 179),
                Color::Rgb(230, 152, 117),
                Color::Rgb(167, 192, 128),
                Color::Rgb(219, 188, 127),
                Color::Rgb(230, 126, 128),
            ),
            (
                TuiTheme::Kanagawa,
                Color::Rgb(149, 127, 184),
                Color::Rgb(230, 195, 132),
                Color::Rgb(152, 187, 108),
                Color::Rgb(255, 158, 59),
                Color::Rgb(232, 36, 36),
            ),
            (
                TuiTheme::Monokai,
                Color::Rgb(102, 217, 239),
                Color::Rgb(253, 151, 31),
                Color::Rgb(166, 226, 46),
                Color::Rgb(230, 219, 116),
                Color::Rgb(249, 38, 114),
            ),
            (
                TuiTheme::OneDark,
                Color::Rgb(97, 175, 239),
                Color::Rgb(209, 154, 102),
                Color::Rgb(152, 195, 121),
                Color::Rgb(229, 192, 123),
                Color::Rgb(224, 108, 117),
            ),
            (
                TuiTheme::AyuDark,
                Color::Rgb(79, 191, 255),
                Color::Rgb(255, 180, 84),
                Color::Rgb(112, 191, 86),
                Color::Rgb(230, 180, 80),
                Color::Rgb(240, 107, 115),
            ),
            (
                TuiTheme::GithubDark,
                Color::Rgb(88, 166, 255),
                Color::Rgb(210, 153, 34),
                Color::Rgb(63, 185, 80),
                Color::Rgb(210, 153, 34),
                Color::Rgb(248, 81, 73),
            ),
        ] {
            let theme = Theme::with_preset(ColorMode::Auto, preset);

            assert_eq!(theme.accent().fg, Some(accent), "{preset:?} accent");
            assert_eq!(theme.key_hint().fg, Some(key_hint), "{preset:?} key hint");
            assert_eq!(
                theme.status(StatusKind::Success).fg,
                Some(success),
                "{preset:?} success"
            );
            assert_eq!(
                theme.status(StatusKind::Warning).fg,
                Some(warning),
                "{preset:?} warning"
            );
            assert_eq!(
                theme.status(StatusKind::Error).fg,
                Some(error),
                "{preset:?} error"
            );
        }
    }

    #[test]
    fn theme_presets_use_quiet_selection_surfaces() {
        for (preset, selected_fg, selected_bg, edit_fg, edit_bg) in [
            (
                TuiTheme::Gruvbox,
                Color::Rgb(250, 189, 47),
                Color::Rgb(60, 56, 54),
                Color::Rgb(235, 219, 178),
                Color::Rgb(80, 73, 69),
            ),
            (
                TuiTheme::RosePine,
                Color::Rgb(235, 188, 186),
                Color::Rgb(64, 61, 82),
                Color::Rgb(224, 222, 244),
                Color::Rgb(38, 35, 58),
            ),
            (
                TuiTheme::SolarizedDark,
                Color::Rgb(38, 139, 210),
                Color::Rgb(7, 54, 66),
                Color::Rgb(131, 148, 150),
                Color::Rgb(7, 54, 66),
            ),
            (
                TuiTheme::SolarizedLight,
                Color::Rgb(38, 139, 210),
                Color::Rgb(238, 232, 213),
                Color::Rgb(101, 123, 131),
                Color::Rgb(238, 232, 213),
            ),
            (
                TuiTheme::GithubDark,
                Color::Rgb(88, 166, 255),
                Color::Rgb(22, 27, 34),
                Color::Rgb(230, 237, 243),
                Color::Rgb(22, 27, 34),
            ),
        ] {
            let theme = Theme::with_preset(ColorMode::Auto, preset);

            assert_eq!(
                theme.selected().fg,
                Some(selected_fg),
                "{preset:?} selected fg"
            );
            assert_eq!(
                theme.selected().bg,
                Some(selected_bg),
                "{preset:?} selected bg"
            );
            assert_eq!(
                theme.edit_focus_row().fg,
                Some(edit_fg),
                "{preset:?} edit fg"
            );
            assert_eq!(
                theme.edit_focus_row().bg,
                Some(edit_bg),
                "{preset:?} edit bg"
            );
        }
    }

    #[test]
    fn no_colour_override_wins_over_configured_theme() {
        let theme = Theme::with_preset(ColorMode::Never, TuiTheme::RosePine);

        assert_eq!(theme.accent().fg, None);
        assert_eq!(theme.key_hint().fg, None);
        assert_eq!(theme.selected().fg, None);
        assert_eq!(theme.selected().bg, None);
        assert!(theme.selected().add_modifier.contains(Modifier::REVERSED));
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

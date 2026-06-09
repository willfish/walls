use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::{app::App, style, terminal_size, TerminalSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartupIntro {
    frame: usize,
    remaining_ticks: u8,
}

impl StartupIntro {
    const TOTAL_TICKS: u8 = 3;
    const SPINNER: [&'static str; 4] = ["|", "/", "-", "\\"];

    pub(crate) fn from_env() -> Self {
        if cfg!(test)
            || std::env::var_os("CI").is_some()
            || intro_disabled_value(std::env::var("WALLS_TUI_INTRO").ok().as_deref())
        {
            Self::disabled()
        } else {
            Self::enabled()
        }
    }

    pub(crate) fn enabled() -> Self {
        Self {
            frame: 0,
            remaining_ticks: Self::TOTAL_TICKS,
        }
    }

    pub(crate) fn disabled() -> Self {
        Self {
            frame: 0,
            remaining_ticks: 0,
        }
    }

    pub(crate) fn is_active(self) -> bool {
        self.remaining_ticks > 0
    }

    pub(crate) fn tick(&mut self) {
        if self.is_active() {
            self.frame += 1;
            self.remaining_ticks = self.remaining_ticks.saturating_sub(1);
        }
    }

    pub(crate) fn skip(&mut self) {
        self.remaining_ticks = 0;
    }

    pub(crate) fn poll_interval(self) -> std::time::Duration {
        if self.is_active() {
            std::time::Duration::from_millis(80)
        } else {
            std::time::Duration::from_millis(200)
        }
    }

    pub(crate) fn spinner(self) -> &'static str {
        Self::SPINNER[self.frame % Self::SPINNER.len()]
    }
}

pub(crate) fn intro_disabled_value(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("0" | "false" | "no" | "off" | "never" | "none" | "skip" | "disabled")
    )
}

pub(crate) fn draw_startup_intro(f: &mut Frame, app: &App, intro: &StartupIntro) {
    let area = f.area();
    if terminal_size(area) == TerminalSize::Tiny {
        return;
    }

    let theme = style::Theme::new(app.color_mode);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("walls")
        .border_style(theme.border())
        .title_style(theme.accent());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let intro_area = centered_rect(inner, 36, 5);
    let paragraph = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("walls", theme.accent()),
            Span::raw(" "),
            Span::styled(intro.spinner(), theme.key_hint()),
        ]),
        Line::from(Span::styled(
            "preparing your wallpaper console",
            theme.muted(),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(paragraph, intro_area);
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

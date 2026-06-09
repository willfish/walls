use std::thread;

use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::{
    app::App,
    layout_size::{terminal_size, TerminalSize},
    style,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StartupIntro {
    frame: usize,
    remaining_ticks: u8,
}

impl StartupIntro {
    const TOTAL_TICKS: u8 = 10;
    const ACTIVE_POLL_MS: u64 = 200;
    const PROGRESS_WIDTH: usize = 18;
    const PHASE_WIDTH: usize = 16;
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
            std::time::Duration::from_millis(Self::ACTIVE_POLL_MS)
        } else {
            std::time::Duration::from_millis(200)
        }
    }

    pub(crate) fn spinner(self) -> &'static str {
        Self::SPINNER[self.frame % Self::SPINNER.len()]
    }

    fn progress(self) -> String {
        let completed = usize::from(Self::TOTAL_TICKS.saturating_sub(self.remaining_ticks));
        let filled = (completed * Self::PROGRESS_WIDTH / usize::from(Self::TOTAL_TICKS))
            .min(Self::PROGRESS_WIDTH);
        format!(
            "[{}{}]",
            "=".repeat(filled),
            " ".repeat(Self::PROGRESS_WIDTH - filled)
        )
    }

    fn phase(self) -> String {
        let phase = match self.frame % 4 {
            0 => "thinking warmly",
            1 => "checking vibes",
            2 => "polishing pixels",
            _ => "one sec pls",
        };
        format!("{phase:<width$}", width = Self::PHASE_WIDTH)
    }
}

pub(crate) fn intro_disabled_value(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("0" | "false" | "no" | "off" | "never" | "none" | "skip" | "disabled")
    )
}

pub(crate) fn start_intro_preview_prewarm(
    app: &App,
    enabled: bool,
) -> Option<thread::JoinHandle<()>> {
    const INTRO_PREWARM_LIMIT: usize = 32;

    if !enabled {
        return None;
    }

    let state = app.ctx.state.clone();
    let cache_dir = app.ctx.paths.cache_dir.clone();
    thread::Builder::new()
        .name("walls-tui-intro-preview-prewarm".into())
        .spawn(move || {
            let sources =
                walls_core::preview_cache::previewable_paths_from_state(&state, &cache_dir)
                    .into_iter()
                    .take(INTRO_PREWARM_LIMIT);
            let stats = walls_core::preview_cache::prewarm_preview_thumbnails(
                sources,
                &cache_dir,
                walls_core::preview_cache::DEFAULT_PREVIEW_SIZE,
            );
            if stats.attempted > 0 {
                tracing::debug!(
                    "startup preview prewarm: attempted={} warmed={} failed={}",
                    stats.attempted,
                    stats.warmed,
                    stats.failed
                );
            }
        })
        .ok()
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

    let intro_area = centered_rect(inner, 42, 7);
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
        Line::from(""),
        Line::from(vec![
            Span::styled(intro.progress(), theme.key_hint()),
            Span::raw(" "),
            Span::styled(intro.phase(), theme.muted()),
        ]),
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

use ratatui::prelude::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalSize {
    Tiny,
    Narrow,
    Standard,
    Wide,
}

pub(crate) fn terminal_size(area: Rect) -> TerminalSize {
    if area.width < 10 || area.height < 6 {
        TerminalSize::Tiny
    } else if area.width < 50 || area.height < 12 {
        TerminalSize::Narrow
    } else if area.width >= 100 && area.height >= 18 {
        TerminalSize::Wide
    } else {
        TerminalSize::Standard
    }
}

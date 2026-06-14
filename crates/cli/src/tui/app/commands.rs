use super::App;
use crate::tui::command::{self, ParsedCommand};
use crate::tui::style::StatusKind;

impl App {
    pub(crate) fn complete_command(&mut self, forward: bool) {
        if let Some(next) = command::complete(&self.cmd_line, forward) {
            self.cmd_line.clear();
            self.cmd_line.push_str(next);
        }
    }

    pub fn run_command(
        &mut self,
        rt: &tokio::runtime::Handle,
    ) -> anyhow::Result<Option<(String, StatusKind)>> {
        let message = match ParsedCommand::parse(&self.cmd_line) {
            ParsedCommand::Next => {
                match tokio::task::block_in_place(|| rt.block_on(self.ctx.advance_next_manual())) {
                    Ok(Some(p)) => (format!("next: {}", p.display()), StatusKind::Success),
                    Ok(None) => (crate::recovery::tui_next_no_change(), StatusKind::Neutral),
                    Err(e) => (crate::recovery::next_error(&e), StatusKind::Error),
                }
            }
            ParsedCommand::Prev => match self.ctx.advance_prev() {
                Ok(Some(p)) => (format!("prev: {}", p.display()), StatusKind::Success),
                Ok(None) => (crate::recovery::tui_no_previous(), StatusKind::Neutral),
                Err(e) => (crate::recovery::prev_error(&e), StatusKind::Error),
            },
            ParsedCommand::TogglePause => {
                self.ctx.toggle_pause()?;
                (
                    format!("paused: {}", self.ctx.state.paused),
                    StatusKind::Success,
                )
            }
            ParsedCommand::Favorite => match self.favorite_current() {
                Ok(msg) => (msg, StatusKind::Success),
                Err(e) => (crate::recovery::favorite_error(&e), StatusKind::Error),
            },
            ParsedCommand::SourceFromCurrent => self.add_wallhaven_source_from_current(rt)?,
            ParsedCommand::Status => (
                format!(
                    "paused={} history={} queue={}",
                    self.ctx.state.paused,
                    self.ctx.state.history.len(),
                    self.ctx.state.cache_queue.len()
                ),
                StatusKind::Neutral,
            ),
            ParsedCommand::Quit => return Ok(None),
            ParsedCommand::Empty => ("(empty command)".into(), StatusKind::Warning),
            ParsedCommand::Unknown(other) => (
                format!(
                    "unknown command: {other} (try :next :prev :pause :favorite :source from-current :status :quit)"
                ),
                StatusKind::Error,
            ),
        };
        Ok(Some(message))
    }
}

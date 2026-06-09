use std::io::{self, stdout, IsTerminal, Stdout};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use anyhow::Context;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::Terminal;

pub(crate) static LOG_BUFFER: Mutex<Vec<String>> = Mutex::new(Vec::new());
static IN_TUI: AtomicBool = AtomicBool::new(false);

const MAX_LOG_LINES: usize = 2000;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub(crate) fn log_len() -> usize {
    LOG_BUFFER.lock().unwrap().len()
}

pub(crate) struct ConsoleWriter;

impl io::Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !IN_TUI.load(Ordering::Relaxed) {
            let _ = io::stderr().write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !IN_TUI.load(Ordering::Relaxed) {
            let _ = io::stderr().flush();
        }
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ConsoleWriter {
    type Writer = ConsoleWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ConsoleWriter
    }
}

pub(crate) struct CaptureWriter;

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            let mut logs = LOG_BUFFER.lock().unwrap();
            for line in s.lines() {
                if !line.trim().is_empty() {
                    logs.push(line.trim_end().to_string());
                    if logs.len() > MAX_LOG_LINES {
                        let to_drain = logs.len() - MAX_LOG_LINES / 2;
                        logs.drain(0..to_drain);
                    }
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter
    }
}

pub(crate) fn enter_terminal() -> anyhow::Result<(TuiTerminal, TerminalRestore)> {
    require_tty()?;

    let mut stdout = stdout();
    enable_raw_mode().context("failed to enable raw mode (is this an interactive terminal?)")?;
    stdout
        .execute(EnterAlternateScreen)
        .context("failed to enter alternate screen")?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    Ok((terminal, TerminalRestore))
}

pub(crate) fn mark_in_tui() {
    IN_TUI.store(true, Ordering::Relaxed);
}

fn require_tty() -> anyhow::Result<()> {
    use std::io::{stdin, stdout};

    if !stdin().is_terminal() || !stdout().is_terminal() {
        anyhow::bail!(
            "walls tui requires an interactive terminal (stdin and stdout must be a TTY).\n\
             Try: walls   # (or `walls tui`) from a terminal emulator, not a pipe or IDE task output"
        );
    }
    Ok(())
}

pub(crate) struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        IN_TUI.store(false, Ordering::Relaxed);
    }
}

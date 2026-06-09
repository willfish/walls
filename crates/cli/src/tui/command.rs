pub(crate) const COMPLETIONS: &[&str] = &["next", "prev", "pause", "favorite", "status", "quit"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParsedCommand<'a> {
    Next,
    Prev,
    TogglePause,
    Favorite,
    Status,
    Quit,
    Empty,
    Unknown(&'a str),
}

impl<'a> ParsedCommand<'a> {
    pub(crate) fn parse(line: &'a str) -> Self {
        match line.trim() {
            "next" | "n" => Self::Next,
            "prev" | "p" => Self::Prev,
            "pause" | "toggle-pause" => Self::TogglePause,
            "favorite" | "fav" | "f" => Self::Favorite,
            "status" => Self::Status,
            "quit" | "q" => Self::Quit,
            "" => Self::Empty,
            other => Self::Unknown(other),
        }
    }
}

pub(crate) fn complete(line: &str, forward: bool) -> Option<&'static str> {
    let prefix = line.trim();
    let exact_command = COMPLETIONS.contains(&prefix);
    let candidates: Vec<&str> = COMPLETIONS
        .iter()
        .copied()
        .filter(|command| exact_command || prefix.is_empty() || command.starts_with(prefix))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    match candidates.iter().position(|command| *command == prefix) {
        Some(index) if forward => Some(candidates[(index + 1) % candidates.len()]),
        Some(index) => Some(candidates[(index + candidates.len() - 1) % candidates.len()]),
        None if forward => Some(candidates[0]),
        None => Some(candidates[candidates.len() - 1]),
    }
}

#[cfg(test)]
mod tests {
    use super::{complete, ParsedCommand};

    #[test]
    fn command_parser_trims_and_maps_dispatch_aliases() {
        assert_eq!(ParsedCommand::parse(" next "), ParsedCommand::Next);
        assert_eq!(ParsedCommand::parse("n"), ParsedCommand::Next);
        assert_eq!(ParsedCommand::parse("prev"), ParsedCommand::Prev);
        assert_eq!(ParsedCommand::parse("p"), ParsedCommand::Prev);
        assert_eq!(ParsedCommand::parse("pause"), ParsedCommand::TogglePause);
        assert_eq!(
            ParsedCommand::parse("toggle-pause"),
            ParsedCommand::TogglePause
        );
        assert_eq!(ParsedCommand::parse("favorite"), ParsedCommand::Favorite);
        assert_eq!(ParsedCommand::parse("fav"), ParsedCommand::Favorite);
        assert_eq!(ParsedCommand::parse("f"), ParsedCommand::Favorite);
        assert_eq!(ParsedCommand::parse("status"), ParsedCommand::Status);
        assert_eq!(ParsedCommand::parse("quit"), ParsedCommand::Quit);
        assert_eq!(ParsedCommand::parse("q"), ParsedCommand::Quit);
    }

    #[test]
    fn command_parser_distinguishes_empty_and_unknown_commands() {
        assert_eq!(ParsedCommand::parse("  "), ParsedCommand::Empty);
        assert_eq!(ParsedCommand::parse("wat"), ParsedCommand::Unknown("wat"));
    }

    #[test]
    fn command_completion_cycles_matching_commands() {
        assert_eq!(complete("", true), Some("next"));
        assert_eq!(complete("p", true), Some("prev"));
        assert_eq!(complete("prev", true), Some("pause"));
        assert_eq!(complete("prev", false), Some("next"));
        assert_eq!(complete("wat", true), None);
    }
}

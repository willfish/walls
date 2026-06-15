use std::path::PathBuf;

use serde_json::json;
use walls_core::providers::ProviderStatusReport;

pub(crate) struct CommandOutcome {
    command: String,
    changed: bool,
    status: String,
    path: Option<PathBuf>,
    exit_code_reason: Option<String>,
}

impl CommandOutcome {
    pub(crate) fn new(
        command: impl Into<String>,
        changed: bool,
        status: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            changed,
            status: status.into(),
            path: None,
            exit_code_reason: None,
        }
    }

    pub(crate) fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub(crate) fn with_exit_code_reason(mut self, reason: impl Into<String>) -> Self {
        self.exit_code_reason = Some(reason.into());
        self
    }

    pub(crate) fn json(self) -> serde_json::Value {
        json!({
            "command": self.command,
            "changed": self.changed,
            "status": self.status,
            "path": self.path.map(|path| path.display().to_string()),
            "exit_code_reason": self.exit_code_reason,
        })
    }
}

pub(crate) fn next_result(
    changed: bool,
    status: &str,
    path: Option<PathBuf>,
    exit_code_reason: Option<&str>,
    provider_report: &ProviderStatusReport,
) -> serde_json::Value {
    json!({
        "command": "next",
        "changed": changed,
        "status": status,
        "path": path.map(|path| path.display().to_string()),
        "exit_code_reason": exit_code_reason,
        "provider_attempts": &provider_report.attempts,
    })
}

#[cfg(test)]
mod tests {
    use super::{next_result, CommandOutcome};
    use walls_core::providers::ProviderStatusReport;

    #[test]
    fn command_outcome_renders_common_envelope() {
        let value = CommandOutcome::new("pause", true, "paused")
            .with_path(std::path::PathBuf::from("/tmp/wall.jpg"))
            .with_exit_code_reason("paused")
            .json();

        assert_eq!(value["command"], "pause");
        assert_eq!(value["changed"], true);
        assert_eq!(value["status"], "paused");
        assert_eq!(value["path"], "/tmp/wall.jpg");
        assert_eq!(value["exit_code_reason"], "paused");
    }

    #[test]
    fn command_outcome_defaults_optional_fields_to_null() {
        let value = CommandOutcome::new("pause", true, "paused").json();

        assert_eq!(value["command"], "pause");
        assert_eq!(value["changed"], true);
        assert_eq!(value["status"], "paused");
        assert!(value["path"].is_null());
        assert!(value["exit_code_reason"].is_null());
    }

    #[test]
    fn next_result_includes_provider_attempts() {
        let report = ProviderStatusReport::default();
        let value = next_result(false, "no_change", None, Some("no_change"), &report);
        assert_eq!(value["command"], "next");
        assert_eq!(value["changed"], false);
        assert_eq!(value["exit_code_reason"], "no_change");
        assert_eq!(value["provider_attempts"].as_array().unwrap().len(), 0);
    }
}

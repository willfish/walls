use std::path::PathBuf;

use serde_json::json;
use walls_core::providers::ProviderStatusReport;

pub(crate) fn command_result(
    command: &str,
    changed: bool,
    status: &str,
    path: Option<PathBuf>,
    exit_code_reason: Option<&str>,
) -> serde_json::Value {
    json!({
        "command": command,
        "changed": changed,
        "status": status,
        "path": path.map(|path| path.display().to_string()),
        "exit_code_reason": exit_code_reason,
    })
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
    use super::{command_result, next_result};
    use walls_core::providers::ProviderStatusReport;

    #[test]
    fn command_result_keeps_common_envelope() {
        let value = command_result("pause", true, "paused", None, None);
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

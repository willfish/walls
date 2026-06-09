use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::apply::ApplyTrigger;
use crate::providers::{
    ProviderAttempt, ProviderAttemptOutcome, ProviderFailureKind, ProviderNoCandidateReason,
};

const MAX_EVENT_TEXT_BYTES: usize = 4096;
const REDACTED_TEXT: &str = "[redacted]";
const SECRET_MARKERS: &[&str] = &[
    "api_key", "apikey", "token", "secret", "password", "bearer ",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    Apply {
        trigger: ApplyTrigger,
        original_path: String,
        composed_path: String,
        provider: Option<String>,
    },
    ApplyFailed {
        trigger: ApplyTrigger,
        original_path: String,
        composed_path: Option<String>,
        provider: Option<String>,
        message: String,
    },
    ProviderAttempt {
        attempt: ProviderAttempt,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub timestamp_unix: u64,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LastRunStatus {
    Applied,
    NoChange,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastRunSummary {
    pub timestamp_unix: u64,
    pub status: LastRunStatus,
    pub message: String,
    pub trigger: Option<ApplyTrigger>,
    pub provider: Option<String>,
    pub applied_path: Option<String>,
    pub composed_path: Option<String>,
    pub no_change_reason: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub provider_attempts: Vec<ProviderAttempt>,
}

impl EventRecord {
    pub fn apply(
        trigger: ApplyTrigger,
        original: &Path,
        composed: &Path,
        provider: Option<String>,
    ) -> Self {
        Self {
            timestamp_unix: now_unix(),
            kind: EventKind::Apply {
                trigger,
                original_path: original.display().to_string(),
                composed_path: composed.display().to_string(),
                provider,
            },
        }
    }

    pub fn provider_attempt(attempt: ProviderAttempt) -> Self {
        Self {
            timestamp_unix: now_unix(),
            kind: EventKind::ProviderAttempt { attempt },
        }
    }

    pub fn apply_failed(
        trigger: ApplyTrigger,
        original: &Path,
        composed: Option<&Path>,
        provider: Option<String>,
        message: String,
    ) -> Self {
        Self {
            timestamp_unix: now_unix(),
            kind: EventKind::ApplyFailed {
                trigger,
                original_path: original.display().to_string(),
                composed_path: composed.map(|path| path.display().to_string()),
                provider,
                message,
            },
        }
    }
}

pub fn append_event(path: &Path, event: &EventRecord) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut value = serde_json::to_value(event)?;
    sanitize_value(&mut value, false);
    let line = serde_json::to_string(&value)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn append_event_best_effort(path: &Path, event: &EventRecord) {
    if let Err(err) = append_event(path, event) {
        tracing::warn!(error = %err, path = %path.display(), "event journal write failed");
    }
}

pub fn read_events(path: &Path) -> anyhow::Result<Vec<EventRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(path)?;
    data.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

pub fn last_run_summary(events: &[EventRecord]) -> Option<LastRunSummary> {
    let latest_terminal = events.iter().enumerate().rev().find(|(_, event)| {
        matches!(
            event.kind,
            EventKind::Apply { .. } | EventKind::ApplyFailed { .. }
        )
    });

    match latest_terminal {
        Some((terminal_index, terminal_event)) => {
            let events_after_terminal = &events[terminal_index + 1..];
            if trailing_provider_run_supersedes_terminal(events_after_terminal) {
                return provider_only_summary(events_after_terminal);
            }
            let previous_terminal_index = events[..terminal_index].iter().rposition(|event| {
                matches!(
                    event.kind,
                    EventKind::Apply { .. } | EventKind::ApplyFailed { .. }
                )
            });
            let segment_start = previous_terminal_index.map_or(0, |index| index + 1);
            match &terminal_event.kind {
                EventKind::Apply { .. } => {
                    applied_summary(terminal_event, &events[segment_start..])
                }
                EventKind::ApplyFailed { .. } => {
                    failed_apply_summary(terminal_event, &events[segment_start..])
                }
                EventKind::ProviderAttempt { .. } => None,
            }
        }
        None => provider_only_summary(events),
    }
}

fn trailing_provider_run_supersedes_terminal(events: &[EventRecord]) -> bool {
    let attempts = provider_attempts(events);
    !attempts.is_empty()
        && !attempts
            .iter()
            .any(|attempt| matches!(attempt.outcome, ProviderAttemptOutcome::Applied { .. }))
}

pub fn journal_path_for_state_file(state_file: &Path) -> PathBuf {
    state_file.with_file_name("events.jsonl")
}

fn applied_summary(apply_event: &EventRecord, events: &[EventRecord]) -> Option<LastRunSummary> {
    let EventKind::Apply {
        trigger,
        original_path,
        composed_path,
        provider,
    } = &apply_event.kind
    else {
        return None;
    };
    let attempts = provider_attempts(events);
    let (warnings, errors) = attempt_messages(&attempts);
    Some(LastRunSummary {
        timestamp_unix: apply_event.timestamp_unix,
        status: LastRunStatus::Applied,
        message: format!(
            "applied {} wallpaper from {}",
            trigger.as_str(),
            provider.as_deref().unwrap_or("local")
        ),
        trigger: Some(*trigger),
        provider: provider.clone(),
        applied_path: Some(original_path.clone()),
        composed_path: Some(composed_path.clone()),
        no_change_reason: None,
        warnings,
        errors,
        provider_attempts: attempts,
    })
}

fn failed_apply_summary(
    failed_event: &EventRecord,
    events: &[EventRecord],
) -> Option<LastRunSummary> {
    let EventKind::ApplyFailed {
        trigger,
        original_path,
        composed_path,
        provider,
        message,
    } = &failed_event.kind
    else {
        return None;
    };
    let attempts = provider_attempts(events);
    let (warnings, mut errors) = attempt_messages(&attempts);
    errors.push(format!("apply backend: {message}"));
    Some(LastRunSummary {
        timestamp_unix: failed_event.timestamp_unix,
        status: LastRunStatus::Failed,
        message: format!("failed to apply {} wallpaper", trigger.as_str()),
        trigger: Some(*trigger),
        provider: provider.clone(),
        applied_path: Some(original_path.clone()),
        composed_path: composed_path.clone(),
        no_change_reason: None,
        warnings,
        errors,
        provider_attempts: attempts,
    })
}

fn provider_only_summary(events: &[EventRecord]) -> Option<LastRunSummary> {
    let attempts = provider_attempts(events);
    if attempts.is_empty() {
        return None;
    }
    let timestamp_unix = events
        .iter()
        .map(|event| event.timestamp_unix)
        .max()
        .unwrap_or(0);
    let (warnings, errors) = attempt_messages(&attempts);
    let failed = attempts
        .iter()
        .any(|attempt| matches!(attempt.outcome, ProviderAttemptOutcome::Failed { .. }));
    let no_change_reason = if failed {
        None
    } else {
        Some("no provider produced a candidate".into())
    };
    Some(LastRunSummary {
        timestamp_unix,
        status: if failed {
            LastRunStatus::Failed
        } else {
            LastRunStatus::NoChange
        },
        message: if failed {
            "failed before applying a wallpaper".into()
        } else {
            "made no wallpaper change".into()
        },
        trigger: None,
        provider: None,
        applied_path: None,
        composed_path: None,
        no_change_reason,
        warnings,
        errors,
        provider_attempts: attempts,
    })
}

fn provider_attempts(events: &[EventRecord]) -> Vec<ProviderAttempt> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ProviderAttempt { attempt } => Some(attempt.clone()),
            EventKind::Apply { .. } | EventKind::ApplyFailed { .. } => None,
        })
        .collect()
}

fn attempt_messages(attempts: &[ProviderAttempt]) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    for attempt in attempts {
        match &attempt.outcome {
            ProviderAttemptOutcome::Skipped { reason }
            | ProviderAttemptOutcome::NoCandidates { reason, .. } => {
                warnings.push(format!(
                    "{}: {}",
                    attempt.provider_id,
                    no_candidate_reason_label(*reason)
                ));
            }
            ProviderAttemptOutcome::Failed {
                kind,
                status_code,
                message,
            } => {
                let mut text = format!("{}: {}", attempt.provider_id, failure_kind_label(*kind));
                if let Some(status_code) = status_code {
                    let _ = write!(text, " HTTP {status_code}");
                }
                if let Some(message) = message {
                    let _ = write!(text, " ({message})");
                }
                errors.push(text);
            }
            ProviderAttemptOutcome::NotRun | ProviderAttemptOutcome::Applied { .. } => {}
        }
    }
    (warnings, errors)
}

fn no_candidate_reason_label(reason: ProviderNoCandidateReason) -> &'static str {
    match reason {
        ProviderNoCandidateReason::Disabled => "disabled",
        ProviderNoCandidateReason::OfflineDisabled => "offline disabled",
        ProviderNoCandidateReason::CredentialMissing => "credential missing",
        ProviderNoCandidateReason::QueueEmpty => "queue empty",
        ProviderNoCandidateReason::NoEnabledSource => "no enabled source",
        ProviderNoCandidateReason::EmptyResult => "empty result",
        ProviderNoCandidateReason::FilteredByHistory => "filtered by history",
        ProviderNoCandidateReason::Unsupported => "unsupported",
    }
}

fn failure_kind_label(kind: ProviderFailureKind) -> &'static str {
    match kind {
        ProviderFailureKind::Request => "request failed",
        ProviderFailureKind::RateLimited => "rate limited",
        ProviderFailureKind::Timeout => "timed out",
        ProviderFailureKind::Connect => "connection failed",
        ProviderFailureKind::Decode => "decode failed",
        ProviderFailureKind::Io => "I/O failed",
        ProviderFailureKind::Config => "config failed",
        ProviderFailureKind::Unknown => "failed",
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn sanitize_value(value: &mut Value, sensitive_key: bool) {
    match value {
        Value::String(text) if sensitive_key || contains_secret_marker(text) => {
            *text = REDACTED_TEXT.to_string();
        }
        Value::String(text) => truncate_text(text),
        Value::Array(items) => {
            for item in items {
                sanitize_value(item, sensitive_key);
            }
        }
        Value::Object(fields) => sanitize_object(fields, sensitive_key),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn sanitize_object(fields: &mut Map<String, Value>, inherited_sensitive_key: bool) {
    for (key, value) in fields {
        let sensitive_key = inherited_sensitive_key || contains_secret_marker(key);
        sanitize_value(value, sensitive_key);
    }
}

fn contains_secret_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|marker| lower.contains(marker))
}

fn truncate_text(text: &mut String) {
    if text.len() <= MAX_EVENT_TEXT_BYTES {
        return;
    }
    let mut boundary = MAX_EVENT_TEXT_BYTES;
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text.truncate(boundary);
}

#[cfg(test)]
mod tests {
    use super::{
        append_event, last_run_summary, read_events, EventKind, EventRecord, LastRunStatus,
    };
    use crate::apply::ApplyTrigger;
    use crate::providers::{
        ProviderAttempt, ProviderAttemptOutcome, ProviderFailureKind, ProviderKind,
        ProviderNoCandidateReason, ProviderOperation, ProviderStatus,
    };

    #[test]
    fn append_event_writes_jsonl_in_order() {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("events.jsonl");

        append_event(
            &journal,
            &EventRecord::apply(
                ApplyTrigger::Manual,
                root.path().join("one.jpg").as_path(),
                root.path().join("wallpaper/one.jpg").as_path(),
                Some("folder".into()),
            ),
        )
        .unwrap();
        append_event(
            &journal,
            &EventRecord::apply(
                ApplyTrigger::Auto,
                root.path().join("two.jpg").as_path(),
                root.path().join("wallpaper/two.jpg").as_path(),
                Some("wallhaven".into()),
            ),
        )
        .unwrap();

        let events = read_events(&journal).unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].kind,
            EventKind::Apply {
                trigger: ApplyTrigger::Manual,
                ..
            }
        ));
        assert!(matches!(
            events[1].kind,
            EventKind::Apply {
                trigger: ApplyTrigger::Auto,
                ..
            }
        ));
    }

    #[test]
    fn append_event_redacts_sensitive_marker_payloads() {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("events.jsonl");
        let attempt = ProviderAttempt {
            provider_id: "wallhaven".into(),
            provider_kind: ProviderKind::Wallhaven,
            operation: ProviderOperation::AdvanceNext,
            status: ProviderStatus::Enabled,
            retries: Vec::new(),
            outcome: ProviderAttemptOutcome::Failed {
                kind: ProviderFailureKind::Request,
                status_code: Some(401),
                message: Some("api_key=super-secret-token".into()),
            },
            fallback_provider_id: None,
        };

        append_event(&journal, &EventRecord::provider_attempt(attempt)).unwrap();

        let raw = std::fs::read_to_string(&journal).unwrap();
        assert!(!raw.contains("super-secret-token"), "{raw}");
        assert!(raw.contains("[redacted]"), "{raw}");
        let events = read_events(&journal).unwrap();
        assert!(matches!(
            &events[0].kind,
            EventKind::ProviderAttempt {
                attempt: ProviderAttempt {
                    outcome: ProviderAttemptOutcome::Failed {
                        message: Some(message),
                        ..
                    },
                    ..
                }
            } if message == "[redacted]"
        ));
    }

    #[test]
    fn append_event_bounds_large_text_fields() {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("events.jsonl");
        let attempt = ProviderAttempt {
            provider_id: "wallhaven".into(),
            provider_kind: ProviderKind::Wallhaven,
            operation: ProviderOperation::AdvanceNext,
            status: ProviderStatus::Enabled,
            retries: Vec::new(),
            outcome: ProviderAttemptOutcome::Failed {
                kind: ProviderFailureKind::Request,
                status_code: Some(500),
                message: Some("x".repeat(super::MAX_EVENT_TEXT_BYTES + 1)),
            },
            fallback_provider_id: None,
        };

        append_event(&journal, &EventRecord::provider_attempt(attempt)).unwrap();

        let events = read_events(&journal).unwrap();
        assert!(matches!(
            &events[0].kind,
            EventKind::ProviderAttempt {
                attempt: ProviderAttempt {
                    outcome: ProviderAttemptOutcome::Failed {
                        message: Some(message),
                        ..
                    },
                    ..
                }
            } if message.len() == super::MAX_EVENT_TEXT_BYTES
        ));
    }

    #[test]
    fn read_events_returns_empty_for_missing_journal() {
        let root = tempfile::tempdir().unwrap();

        let events = read_events(&root.path().join("missing.jsonl")).unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn last_run_summary_reports_applied_wallpaper_with_provider_context() {
        let root = tempfile::tempdir().unwrap();
        let events = vec![
            EventRecord::provider_attempt(ProviderAttempt {
                provider_id: "wallhaven".into(),
                provider_kind: ProviderKind::Wallhaven,
                operation: ProviderOperation::AdvanceNext,
                status: ProviderStatus::Enabled,
                retries: Vec::new(),
                outcome: ProviderAttemptOutcome::NoCandidates {
                    reason: ProviderNoCandidateReason::QueueEmpty,
                    candidate_count: Some(0),
                },
                fallback_provider_id: Some("local".into()),
            }),
            EventRecord::apply(
                ApplyTrigger::Manual,
                root.path().join("one.jpg").as_path(),
                root.path().join("wallpaper/one.jpg").as_path(),
                Some("folder".into()),
            ),
            EventRecord::provider_attempt(ProviderAttempt {
                provider_id: "local".into(),
                provider_kind: ProviderKind::Local,
                operation: ProviderOperation::AdvanceNext,
                status: ProviderStatus::Enabled,
                retries: Vec::new(),
                outcome: ProviderAttemptOutcome::Applied {
                    candidate_count: Some(2),
                },
                fallback_provider_id: None,
            }),
        ];

        let summary = last_run_summary(&events).expect("summary");

        assert_eq!(summary.status, LastRunStatus::Applied);
        assert_eq!(summary.trigger, Some(ApplyTrigger::Manual));
        assert_eq!(summary.provider.as_deref(), Some("folder"));
        assert!(summary.applied_path.unwrap().ends_with("one.jpg"));
        assert_eq!(summary.provider_attempts.len(), 2);
        assert_eq!(summary.warnings, ["wallhaven: queue empty"]);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn last_run_summary_reports_no_change_when_providers_have_no_candidates() {
        let events = vec![EventRecord::provider_attempt(ProviderAttempt {
            provider_id: "local".into(),
            provider_kind: ProviderKind::Local,
            operation: ProviderOperation::AdvanceNext,
            status: ProviderStatus::Enabled,
            retries: Vec::new(),
            outcome: ProviderAttemptOutcome::NoCandidates {
                reason: ProviderNoCandidateReason::EmptyResult,
                candidate_count: Some(0),
            },
            fallback_provider_id: None,
        })];

        let summary = last_run_summary(&events).expect("summary");

        assert_eq!(summary.status, LastRunStatus::NoChange);
        assert_eq!(
            summary.no_change_reason.as_deref(),
            Some("no provider produced a candidate")
        );
        assert_eq!(summary.message, "made no wallpaper change");
        assert_eq!(summary.warnings, ["local: empty result"]);
    }

    #[test]
    fn last_run_summary_prefers_trailing_no_change_run_over_previous_apply() {
        let root = tempfile::tempdir().unwrap();
        let events = vec![
            EventRecord::provider_attempt(ProviderAttempt {
                provider_id: "local".into(),
                provider_kind: ProviderKind::Local,
                operation: ProviderOperation::AdvanceNext,
                status: ProviderStatus::Enabled,
                retries: Vec::new(),
                outcome: ProviderAttemptOutcome::Applied {
                    candidate_count: Some(1),
                },
                fallback_provider_id: None,
            }),
            EventRecord::apply(
                ApplyTrigger::Auto,
                root.path().join("one.jpg").as_path(),
                root.path().join("wallpaper/one.jpg").as_path(),
                Some("local".into()),
            ),
            EventRecord::provider_attempt(ProviderAttempt {
                provider_id: "local".into(),
                provider_kind: ProviderKind::Local,
                operation: ProviderOperation::AdvanceNext,
                status: ProviderStatus::Enabled,
                retries: Vec::new(),
                outcome: ProviderAttemptOutcome::NoCandidates {
                    reason: ProviderNoCandidateReason::EmptyResult,
                    candidate_count: Some(0),
                },
                fallback_provider_id: None,
            }),
        ];

        let summary = last_run_summary(&events).expect("summary");

        assert_eq!(summary.status, LastRunStatus::NoChange);
        assert_eq!(summary.message, "made no wallpaper change");
        assert_eq!(summary.provider_attempts.len(), 1);
        assert!(summary.applied_path.is_none());
    }

    #[test]
    fn last_run_summary_reports_failure_without_leaking_redacted_messages() {
        let events = vec![EventRecord::provider_attempt(ProviderAttempt {
            provider_id: "wallhaven".into(),
            provider_kind: ProviderKind::Wallhaven,
            operation: ProviderOperation::AdvanceNext,
            status: ProviderStatus::Enabled,
            retries: Vec::new(),
            outcome: ProviderAttemptOutcome::Failed {
                kind: ProviderFailureKind::Request,
                status_code: Some(401),
                message: Some("[redacted]".into()),
            },
            fallback_provider_id: Some("local".into()),
        })];

        let summary = last_run_summary(&events).expect("summary");
        let raw = serde_json::to_string(&summary).unwrap();

        assert_eq!(summary.status, LastRunStatus::Failed);
        assert_eq!(
            summary.errors,
            ["wallhaven: request failed HTTP 401 ([redacted])"]
        );
        assert!(!raw.contains("super-secret-token"), "{raw}");
    }

    #[test]
    fn last_run_summary_reports_apply_backend_failure() {
        let root = tempfile::tempdir().unwrap();
        let event = EventRecord::apply_failed(
            ApplyTrigger::Manual,
            root.path().join("one.jpg").as_path(),
            Some(root.path().join("wallpaper/one.jpg").as_path()),
            Some("local".into()),
            "custom apply script failed".into(),
        );

        let summary = last_run_summary(&[event]).expect("summary");

        assert_eq!(summary.status, LastRunStatus::Failed);
        assert_eq!(summary.trigger, Some(ApplyTrigger::Manual));
        assert_eq!(summary.provider.as_deref(), Some("local"));
        assert_eq!(
            summary.errors,
            ["apply backend: custom apply script failed"]
        );
    }
}

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::apply::ApplyTrigger;
use crate::providers::ProviderAttempt;

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

pub fn journal_path_for_state_file(state_file: &Path) -> PathBuf {
    state_file.with_file_name("events.jsonl")
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
    use super::{append_event, read_events, EventKind, EventRecord};
    use crate::apply::ApplyTrigger;
    use crate::providers::{
        ProviderAttempt, ProviderAttemptOutcome, ProviderFailureKind, ProviderKind,
        ProviderOperation, ProviderStatus,
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
}

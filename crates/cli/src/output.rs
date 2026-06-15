use std::path::PathBuf;

use serde_json::json;
use walls_core::downloads::{
    CacheFileEntry, CacheInspection, NukeDownloadsMode, NukeDownloadsPlan, NukeDownloadsResult,
};
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

pub(crate) struct CacheCommandOutcome {
    command: String,
    changed: bool,
    status: String,
    exit_code_reason: Option<String>,
    payload: CacheCommandPayload,
}

enum CacheCommandPayload {
    Status {
        cache_dir: PathBuf,
        download_dir: PathBuf,
        inspection: CacheInspection,
        quota_enabled: bool,
        quota_size_mb: u64,
    },
    Plan {
        dry_run: bool,
        plan: NukeDownloadsPlan,
        cache_dir: PathBuf,
        download_dir: PathBuf,
    },
    Result {
        result: NukeDownloadsResult,
    },
    QueueResult {
        queue_cleared: usize,
    },
    ForceRequired {
        message: String,
    },
    Inspect {
        provider: Option<String>,
        files: Vec<CacheFileEntry>,
    },
}

impl CacheCommandOutcome {
    pub(crate) fn status(
        command: impl Into<String>,
        cache_dir: PathBuf,
        download_dir: PathBuf,
        inspection: CacheInspection,
        quota_enabled: bool,
        quota_size_mb: u64,
    ) -> Self {
        Self {
            command: command.into(),
            changed: false,
            status: "ok".into(),
            exit_code_reason: None,
            payload: CacheCommandPayload::Status {
                cache_dir,
                download_dir,
                inspection,
                quota_enabled,
                quota_size_mb,
            },
        }
    }

    pub(crate) fn plan(
        command: impl Into<String>,
        status: impl Into<String>,
        dry_run: bool,
        plan: NukeDownloadsPlan,
        cache_dir: PathBuf,
        download_dir: PathBuf,
    ) -> Self {
        Self {
            command: command.into(),
            changed: false,
            status: status.into(),
            exit_code_reason: None,
            payload: CacheCommandPayload::Plan {
                dry_run,
                plan,
                cache_dir,
                download_dir,
            },
        }
    }

    pub(crate) fn result(command: impl Into<String>, result: NukeDownloadsResult) -> Self {
        let changed =
            result.queue_cleared > 0 || result.cache_removed > 0 || result.download_removed > 0;
        Self {
            command: command.into(),
            changed,
            status: cache_result_status(&result).into(),
            exit_code_reason: None,
            payload: CacheCommandPayload::Result { result },
        }
    }

    pub(crate) fn queue_result(command: impl Into<String>, queue_cleared: usize) -> Self {
        Self {
            command: command.into(),
            changed: queue_cleared > 0,
            status: if queue_cleared > 0 {
                "cleared_queue".into()
            } else {
                "noop".into()
            },
            exit_code_reason: None,
            payload: CacheCommandPayload::QueueResult { queue_cleared },
        }
    }

    pub(crate) fn force_required(command: impl Into<String>) -> Self {
        let command = command.into();
        let message =
            format!("{command}: refusing to mutate without --force; use --dry-run to preview");
        Self {
            command,
            changed: false,
            status: "force_required".into(),
            exit_code_reason: Some("force_required".into()),
            payload: CacheCommandPayload::ForceRequired { message },
        }
    }

    pub(crate) fn inspect(
        command: impl Into<String>,
        provider: Option<String>,
        files: Vec<CacheFileEntry>,
    ) -> Self {
        Self {
            command: command.into(),
            changed: false,
            status: "ok".into(),
            exit_code_reason: None,
            payload: CacheCommandPayload::Inspect { provider, files },
        }
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        match &self.payload {
            CacheCommandPayload::Status {
                cache_dir,
                download_dir,
                inspection,
                quota_enabled,
                quota_size_mb,
            } => {
                let quota_bytes = quota_size_mb.saturating_mul(1024 * 1024);
                json!({
                    "command": self.command,
                    "changed": self.changed,
                    "status": self.status,
                    "paths": {
                        "cache_dir": cache_dir.display().to_string(),
                        "download_dir": download_dir.display().to_string(),
                    },
                    "queue": {
                        "len": inspection.queue_len,
                        "ids": inspection.queue_ids,
                    },
                    "cache": {
                        "files": inspection.cache.files,
                        "bytes": inspection.cache.bytes,
                        "provider_files": inspection.cache.provider_files,
                        "provider_bytes": inspection.cache.provider_bytes,
                    },
                    "downloads": {
                        "files": inspection.downloads.files,
                        "bytes": inspection.downloads.bytes,
                        "provider_files": inspection.downloads.provider_files,
                        "provider_bytes": inspection.downloads.provider_bytes,
                    },
                    "quota": {
                        "enabled": quota_enabled,
                        "size_mb": quota_size_mb,
                        "size_bytes": quota_bytes,
                        "usage_bytes": inspection.downloads.bytes,
                        "over_quota": *quota_enabled && quota_bytes > 0 && inspection.downloads.bytes > quota_bytes,
                    },
                    "state_references": {
                        "current_provider_storage": inspection.current_provider_storage,
                        "history_provider_entries": inspection.history_provider_entries,
                    },
                    "exit_code_reason": self.exit_code_reason,
                })
            }
            CacheCommandPayload::Plan {
                dry_run,
                plan,
                cache_dir,
                download_dir,
            } => json!({
                "command": self.command,
                "changed": self.changed,
                "status": self.status,
                "dry_run": dry_run,
                "plan": {
                    "mode": plan.mode.label(),
                    "queue_len": plan.queue_len,
                    "cache_files": plan.cache_files,
                    "download_files": plan.download_files,
                    "history_provider_entries": plan.history_provider_entries,
                    "current_provider_storage": plan.current_provider_storage,
                    "cache_dir": cache_dir.display().to_string(),
                    "download_dir": download_dir.display().to_string(),
                },
                "exit_code_reason": self.exit_code_reason,
            }),
            CacheCommandPayload::Result { result } => json!({
                "command": self.command,
                "changed": self.changed,
                "status": self.status,
                "mode": result.mode.label(),
                "queue_cleared": result.queue_cleared,
                "cache_removed": result.cache_removed,
                "download_removed": result.download_removed,
                "history_pruned": result.history_pruned,
                "current_cleared": result.current_cleared,
                "exit_code_reason": self.exit_code_reason,
            }),
            CacheCommandPayload::QueueResult { queue_cleared } => json!({
                "command": self.command,
                "changed": self.changed,
                "status": self.status,
                "queue_cleared": queue_cleared,
                "exit_code_reason": self.exit_code_reason,
            }),
            CacheCommandPayload::ForceRequired { message } => json!({
                "command": self.command,
                "changed": self.changed,
                "status": self.status,
                "message": message,
                "exit_code_reason": self.exit_code_reason,
            }),
            CacheCommandPayload::Inspect { provider, files } => json!({
                "command": self.command,
                "changed": self.changed,
                "status": self.status,
                "provider": provider,
                "files": files.iter().map(cache_file_json).collect::<Vec<_>>(),
                "exit_code_reason": self.exit_code_reason,
            }),
        }
    }

    pub(crate) fn human_lines(&self) -> Vec<String> {
        match &self.payload {
            CacheCommandPayload::Status {
                cache_dir,
                download_dir,
                inspection,
                quota_enabled,
                quota_size_mb,
            } => vec![
                format!("cache dir: {}", cache_dir.display()),
                format!(
                    "cache files: {} provider / {} total ({} bytes provider / {} bytes total)",
                    inspection.cache.provider_files,
                    inspection.cache.files,
                    inspection.cache.provider_bytes,
                    inspection.cache.bytes
                ),
                format!("download dir: {}", download_dir.display()),
                format!(
                    "download files: {} files ({} bytes)",
                    inspection.downloads.files, inspection.downloads.bytes
                ),
                format!("queue: {} entries", inspection.queue_len),
                format!(
                    "quota: {} ({} MiB, {} bytes used{})",
                    if *quota_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    },
                    quota_size_mb,
                    inspection.downloads.bytes,
                    quota_suffix(*quota_enabled, *quota_size_mb, inspection.downloads.bytes)
                ),
                format!(
                    "provider state references: current={}, history={}",
                    inspection.current_provider_storage, inspection.history_provider_entries
                ),
            ],
            CacheCommandPayload::Plan { plan, .. } => cache_plan_human_lines(plan),
            CacheCommandPayload::Result { result } => cache_result_human_lines(result),
            CacheCommandPayload::QueueResult { queue_cleared } => {
                if *queue_cleared > 0 {
                    vec![format!("cleared queue: {queue_cleared} entries")]
                } else {
                    vec!["nothing to clear".into()]
                }
            }
            CacheCommandPayload::ForceRequired { message } => vec![message.clone()],
            CacheCommandPayload::Inspect { files, .. } => {
                if files.is_empty() {
                    return vec!["no provider cache files".into()];
                }
                files
                    .iter()
                    .map(|file| {
                        format!(
                            "{}\t{}\t{}\t{}",
                            file.area.label(),
                            file.provider.as_deref().unwrap_or("unknown"),
                            file.bytes,
                            file.path.display()
                        )
                    })
                    .collect()
            }
        }
    }
}

fn quota_suffix(enabled: bool, size_mb: u64, bytes: u64) -> String {
    if !enabled {
        return String::new();
    }
    let limit = size_mb.saturating_mul(1024 * 1024);
    if limit == 0 {
        return String::from(", no valid quota limit");
    }
    if bytes > limit {
        format!(", {} bytes over quota", bytes - limit)
    } else {
        format!(", {} bytes remaining", limit - bytes)
    }
}

fn cache_file_json(file: &CacheFileEntry) -> serde_json::Value {
    json!({
        "area": file.area.label(),
        "name": file.name,
        "path": file.path.display().to_string(),
        "bytes": file.bytes,
        "provider": file.provider,
    })
}

fn cache_plan_human_lines(plan: &NukeDownloadsPlan) -> Vec<String> {
    match plan.mode {
        NukeDownloadsMode::ClearQueue => {
            vec![format!("would clear queue: {} entries", plan.queue_len)]
        }
        NukeDownloadsMode::PurgeProviderFiles => {
            vec![format!(
                "would purge provider files: {} cache files, {} downloaded files",
                plan.cache_files, plan.download_files
            )]
        }
        NukeDownloadsMode::ProviderReset => {
            vec![format!(
                "would reset provider storage: {} queued, {} cache files, {} downloaded files, {} history entries, current={}",
                plan.queue_len,
                plan.cache_files,
                plan.download_files,
                plan.history_provider_entries,
                plan.current_provider_storage
            )]
        }
        NukeDownloadsMode::Nothing => vec!["nothing to prune".into()],
    }
}

fn cache_result_human_lines(result: &NukeDownloadsResult) -> Vec<String> {
    match result.mode {
        NukeDownloadsMode::ClearQueue => {
            vec![format!("cleared queue: {} entries", result.queue_cleared)]
        }
        NukeDownloadsMode::PurgeProviderFiles => {
            vec![format!(
                "purged provider files: {} cache files, {} downloaded files",
                result.cache_removed, result.download_removed
            )]
        }
        NukeDownloadsMode::ProviderReset => {
            vec![format!(
                "reset provider storage: {} queued, {} cache files, {} downloaded files, {} history entries, current={}",
                result.queue_cleared,
                result.cache_removed,
                result.download_removed,
                result.history_pruned,
                result.current_cleared
            )]
        }
        NukeDownloadsMode::Nothing => vec!["nothing to prune".into()],
    }
}

fn cache_result_status(result: &NukeDownloadsResult) -> &'static str {
    match result.mode {
        NukeDownloadsMode::ClearQueue if result.queue_cleared > 0 => "cleared_queue",
        NukeDownloadsMode::PurgeProviderFiles
            if result.cache_removed > 0 || result.download_removed > 0 =>
        {
            "purged_provider_files"
        }
        NukeDownloadsMode::ProviderReset
            if result.queue_cleared > 0
                || result.cache_removed > 0
                || result.download_removed > 0
                || result.history_pruned > 0
                || result.current_cleared =>
        {
            "reset_provider_storage"
        }
        _ => "noop",
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
    use super::{next_result, CacheCommandOutcome, CommandOutcome};
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

    #[test]
    fn cache_plan_outcome_renders_common_envelope_and_plan() {
        let plan = walls_core::downloads::NukeDownloadsPlan {
            mode: walls_core::downloads::NukeDownloadsMode::ProviderReset,
            queue_len: 2,
            cache_files: 3,
            download_files: 5,
            history_provider_entries: 7,
            current_provider_storage: true,
        };

        let value = CacheCommandOutcome::plan(
            "cache prune",
            "would_reset_provider_storage",
            true,
            plan,
            std::path::PathBuf::from("/tmp/cache"),
            std::path::PathBuf::from("/tmp/downloads"),
        )
        .json();

        assert_eq!(value["command"], "cache prune");
        assert_eq!(value["changed"], false);
        assert_eq!(value["status"], "would_reset_provider_storage");
        assert_eq!(value["dry_run"], true);
        assert_eq!(value["plan"]["mode"], "provider_reset");
        assert_eq!(value["plan"]["queue_len"], 2);
        assert_eq!(value["plan"]["cache_files"], 3);
        assert_eq!(value["plan"]["download_files"], 5);
        assert_eq!(value["plan"]["history_provider_entries"], 7);
        assert_eq!(value["plan"]["current_provider_storage"], true);
        assert_eq!(value["plan"]["cache_dir"], "/tmp/cache");
        assert_eq!(value["plan"]["download_dir"], "/tmp/downloads");
        assert!(value["exit_code_reason"].is_null());
    }
}

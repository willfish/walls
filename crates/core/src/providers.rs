use crate::config::{Config, Secrets, SourceEntry, SourceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    Wallhaven,
    Unsplash,
    Reddit,
    Bing,
    Apod,
    MediaRss,
    Attribution,
    Json,
    Pixabay,
    Immich,
    Spotlight,
    Weighting,
    Unsupported,
}

impl ProviderKind {
    pub fn is_local(self) -> bool {
        self == Self::Local
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCapability {
    ConfigValidation,
    QueueRefill,
    Download,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: String,
    pub kind: ProviderKind,
    pub enabled: bool,
    pub capabilities: Vec<ProviderCapability>,
}

impl ProviderDescriptor {
    pub fn failure_scope(&self, operation: &'static str) -> ProviderFailureScope {
        ProviderFailureScope {
            provider_id: self.id.clone(),
            provider_kind: self.kind,
            operation,
        }
    }

    pub fn attempt(&self, operation: ProviderOperation) -> ProviderAttempt {
        ProviderAttempt::new(self, operation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFailureScope {
    pub provider_id: String,
    pub provider_kind: ProviderKind,
    pub operation: &'static str,
}

impl std::fmt::Display for ProviderFailureScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "provider {} ({:?}) failed during {}",
            self.provider_id, self.provider_kind, self.operation
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Enabled,
    Disabled,
    OfflineDisabled,
    CredentialMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperation {
    AdvanceNext,
    QueueRefill,
    Search,
    Download,
    Metadata,
    DoctorCheck,
    LocalSourceListing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderNoCandidateReason {
    Disabled,
    OfflineDisabled,
    CredentialMissing,
    QueueEmpty,
    NoEnabledSource,
    EmptyResult,
    FilteredByHistory,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureKind {
    Request,
    RateLimited,
    Timeout,
    Connect,
    Decode,
    Io,
    Config,
    Unknown,
}

impl ProviderFailureKind {
    pub fn classify(error: &anyhow::Error) -> (Self, Option<u16>) {
        for cause in error.chain() {
            if let Some(reqwest) = cause.downcast_ref::<reqwest::Error>() {
                let status = reqwest.status().map(|status| status.as_u16());
                if status == Some(429) {
                    return (Self::RateLimited, status);
                }
                if reqwest.is_timeout() {
                    return (Self::Timeout, status);
                }
                if reqwest.is_connect() {
                    return (Self::Connect, status);
                }
                if status.is_some() || reqwest.is_request() {
                    return (Self::Request, status);
                }
                if reqwest.is_decode() {
                    return (Self::Decode, status);
                }
            }
            if cause.downcast_ref::<std::io::Error>().is_some() {
                return (Self::Io, None);
            }
            if cause.downcast_ref::<serde_json::Error>().is_some() {
                return (Self::Decode, None);
            }
        }
        (Self::Unknown, None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRetryReason {
    RateLimited,
    ServerError,
    Timeout,
    Connect,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderRetry {
    pub attempt: u32,
    pub backoff_ms: u64,
    pub reason: ProviderRetryReason,
    pub status_code: Option<u16>,
}

impl ProviderRetry {
    pub fn rate_limited(attempt: u32, backoff_ms: u64) -> Self {
        Self {
            attempt,
            backoff_ms,
            reason: ProviderRetryReason::RateLimited,
            status_code: Some(429),
        }
    }

    pub fn server_error(attempt: u32, backoff_ms: u64, status_code: u16) -> Self {
        Self {
            attempt,
            backoff_ms,
            reason: ProviderRetryReason::ServerError,
            status_code: Some(status_code),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ProviderAttemptOutcome {
    NotRun,
    Applied {
        candidate_count: Option<usize>,
    },
    Skipped {
        reason: ProviderNoCandidateReason,
    },
    NoCandidates {
        reason: ProviderNoCandidateReason,
        candidate_count: Option<usize>,
    },
    Failed {
        kind: ProviderFailureKind,
        status_code: Option<u16>,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderAttempt {
    pub provider_id: String,
    pub provider_kind: ProviderKind,
    pub operation: ProviderOperation,
    pub status: ProviderStatus,
    pub retries: Vec<ProviderRetry>,
    pub outcome: ProviderAttemptOutcome,
    pub fallback_provider_id: Option<String>,
}

impl ProviderAttempt {
    pub fn new(provider: &ProviderDescriptor, operation: ProviderOperation) -> Self {
        Self {
            provider_id: provider.id.clone(),
            provider_kind: provider.kind,
            operation,
            status: if provider.enabled {
                ProviderStatus::Enabled
            } else {
                ProviderStatus::Disabled
            },
            retries: Vec::new(),
            outcome: ProviderAttemptOutcome::NotRun,
            fallback_provider_id: None,
        }
    }

    #[must_use]
    pub fn with_status(mut self, status: ProviderStatus) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn with_retry(mut self, retry: ProviderRetry) -> Self {
        self.retries.push(retry);
        self
    }

    #[must_use]
    pub fn with_retries(mut self, retries: impl IntoIterator<Item = ProviderRetry>) -> Self {
        self.retries.extend(retries);
        self
    }

    #[must_use]
    pub fn with_fallback(mut self, provider_id: impl Into<String>) -> Self {
        self.fallback_provider_id = Some(provider_id.into());
        self
    }

    #[must_use]
    pub fn skipped(mut self, reason: ProviderNoCandidateReason) -> Self {
        self.outcome = ProviderAttemptOutcome::Skipped { reason };
        self
    }

    #[must_use]
    pub fn no_candidates(
        mut self,
        reason: ProviderNoCandidateReason,
        candidate_count: Option<usize>,
    ) -> Self {
        self.outcome = ProviderAttemptOutcome::NoCandidates {
            reason,
            candidate_count,
        };
        self
    }

    #[must_use]
    pub fn applied(mut self, candidate_count: Option<usize>) -> Self {
        self.outcome = ProviderAttemptOutcome::Applied { candidate_count };
        self
    }

    #[must_use]
    pub fn failed(
        mut self,
        kind: ProviderFailureKind,
        status_code: Option<u16>,
        message: Option<String>,
    ) -> Self {
        self.outcome = ProviderAttemptOutcome::Failed {
            kind,
            status_code,
            message,
        };
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderStatusReport {
    pub attempts: Vec<ProviderAttempt>,
}

impl ProviderStatusReport {
    pub fn push(&mut self, attempt: ProviderAttempt) {
        self.attempts.push(attempt);
    }

    pub fn attempted_provider(&self, provider_id: &str) -> bool {
        self.attempts
            .iter()
            .any(|attempt| attempt.provider_id == provider_id)
    }
}

pub fn provider_for_source(source: &SourceEntry) -> ProviderDescriptor {
    let kind = provider_kind(SourceKind::parse(&source.source_type));
    ProviderDescriptor {
        id: source
            .label
            .clone()
            .unwrap_or_else(|| source.source_type.clone()),
        kind,
        enabled: source.enabled,
        capabilities: capabilities_for_kind(kind),
    }
}

pub fn configured_source_providers(sources: &[SourceEntry]) -> Vec<ProviderDescriptor> {
    sources.iter().map(provider_for_source).collect()
}

pub fn configured_providers(config: &Config, _secrets: &Secrets) -> Vec<ProviderDescriptor> {
    configured_source_providers(&config.sources)
}

pub fn enabled_local_sources(sources: &[SourceEntry]) -> impl Iterator<Item = &SourceEntry> {
    sources
        .iter()
        .filter(|source| source.enabled && SourceKind::parse(&source.source_type).is_local())
}

pub fn wallhaven_provider(config: &Config, _secrets: &Secrets) -> ProviderDescriptor {
    let source_enabled = config.sources.iter().any(|source| {
        source.enabled && SourceKind::parse(&source.source_type) == SourceKind::Wallhaven
    });
    ProviderDescriptor {
        id: "wallhaven".into(),
        kind: ProviderKind::Wallhaven,
        enabled: config.change.internet_enabled && source_enabled,
        capabilities: capabilities_for_kind(ProviderKind::Wallhaven),
    }
}

pub fn unsplash_provider(config: &Config, secrets: &Secrets) -> ProviderDescriptor {
    ProviderDescriptor {
        id: "unsplash".into(),
        kind: ProviderKind::Unsplash,
        enabled: config.change.internet_enabled
            && !secrets.unsplash_access_key.is_empty()
            && config.sources.iter().any(|source| {
                source.enabled && SourceKind::parse(&source.source_type) == SourceKind::Unsplash
            }),
        capabilities: capabilities_for_kind(ProviderKind::Unsplash),
    }
}

fn provider_kind(source_kind: SourceKind) -> ProviderKind {
    match source_kind {
        SourceKind::Folder | SourceKind::Favorites | SourceKind::Fetched | SourceKind::Image => {
            ProviderKind::Local
        }
        SourceKind::Unsplash => ProviderKind::Unsplash,
        SourceKind::Reddit => ProviderKind::Reddit,
        SourceKind::Bing => ProviderKind::Bing,
        SourceKind::Apod => ProviderKind::Apod,
        SourceKind::MediaRss => ProviderKind::MediaRss,
        SourceKind::Attribution => ProviderKind::Attribution,
        SourceKind::Json => ProviderKind::Json,
        SourceKind::Pixabay => ProviderKind::Pixabay,
        SourceKind::Immich => ProviderKind::Immich,
        SourceKind::Spotlight => ProviderKind::Spotlight,
        SourceKind::Weighting => ProviderKind::Weighting,
        SourceKind::Wallhaven => ProviderKind::Wallhaven,
        SourceKind::Unknown => ProviderKind::Unsupported,
    }
}

fn capabilities_for_kind(kind: ProviderKind) -> Vec<ProviderCapability> {
    match kind {
        ProviderKind::Local => vec![ProviderCapability::ConfigValidation],
        ProviderKind::Wallhaven
        | ProviderKind::Unsplash
        | ProviderKind::Reddit
        | ProviderKind::Bing
        | ProviderKind::Apod
        | ProviderKind::MediaRss
        | ProviderKind::Attribution
        | ProviderKind::Json
        | ProviderKind::Pixabay
        | ProviderKind::Immich
        | ProviderKind::Spotlight
        | ProviderKind::Weighting => vec![
            ProviderCapability::ConfigValidation,
            ProviderCapability::QueueRefill,
            ProviderCapability::Download,
            ProviderCapability::Metadata,
        ],
        ProviderKind::Unsupported => Vec::new(),
    }
}

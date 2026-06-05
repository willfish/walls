use crate::config::{Config, Secrets, SourceEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn provider_for_source(source: &SourceEntry) -> ProviderDescriptor {
    let kind = source_kind(&source.source_type);
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

pub fn configured_providers(config: &Config, secrets: &Secrets) -> Vec<ProviderDescriptor> {
    let mut providers = configured_source_providers(&config.sources);
    providers.push(wallhaven_provider(config, secrets));
    providers
}

pub fn enabled_local_sources(sources: &[SourceEntry]) -> impl Iterator<Item = &SourceEntry> {
    sources
        .iter()
        .filter(|source| source.enabled && source_kind(&source.source_type).is_local())
}

pub fn wallhaven_provider(config: &Config, secrets: &Secrets) -> ProviderDescriptor {
    ProviderDescriptor {
        id: "wallhaven".into(),
        kind: ProviderKind::Wallhaven,
        enabled: config.change.internet_enabled && !secrets.wallhaven_api_key.is_empty(),
        capabilities: capabilities_for_kind(ProviderKind::Wallhaven),
    }
}

pub fn unsplash_provider(config: &Config, secrets: &Secrets) -> ProviderDescriptor {
    ProviderDescriptor {
        id: "unsplash".into(),
        kind: ProviderKind::Unsplash,
        enabled: config.change.internet_enabled
            && !secrets.unsplash_access_key.is_empty()
            && config
                .sources
                .iter()
                .any(|source| source.enabled && source.source_type == "unsplash"),
        capabilities: capabilities_for_kind(ProviderKind::Unsplash),
    }
}

fn source_kind(source_type: &str) -> ProviderKind {
    match source_type {
        "folder" | "favorites" | "fetched" | "image" => ProviderKind::Local,
        "unsplash" => ProviderKind::Unsplash,
        "reddit" => ProviderKind::Reddit,
        "bing" => ProviderKind::Bing,
        "apod" => ProviderKind::Apod,
        "mediarss" => ProviderKind::MediaRss,
        "attribution" => ProviderKind::Attribution,
        "json" => ProviderKind::Json,
        "pixabay" => ProviderKind::Pixabay,
        "immich" => ProviderKind::Immich,
        "spotlight" => ProviderKind::Spotlight,
        "weighting" => ProviderKind::Weighting,
        _ => ProviderKind::Unsupported,
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

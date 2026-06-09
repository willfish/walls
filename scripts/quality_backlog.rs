use std::collections::HashMap;
use std::time::Duration;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BacklogError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub enabled: bool,
    pub search_config: Option<SearchConfig>,
    pub source_schemas: Vec<String>,
    pub timeout_seconds: u64,
    pub retry_policy: RetryPolicy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchConfig {
    pub query: String,
    pub max_results: u32,
    pub sort_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

pub struct QualityBacklog {
    client: Client,
    providers: HashMap<String, ProviderConfig>,
}

impl QualityBacklog {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            providers: HashMap::new(),
        }
    }

    pub fn load_providers(&mut self, config_path: &str) -> Result<(), BacklogError> {
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| BacklogError::ConfigError(format!("Failed to read config: {}", e)))?;

        let providers: Vec<ProviderConfig> = serde_json::from_str(&config_content)
            .map_err(|e| BacklogError::ConfigError(format!("Failed to parse config: {}", e)))?;

        for provider in providers {
            self.providers.insert(provider.name.clone(), provider);
        }

        Ok(())
    }

    pub fn validate_wallhaven_search_config(&self, provider_name: &str) -> Result<(), BacklogError> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| BacklogError::ValidationError(format!("Provider '{}' not found", provider_name)))?;

        if let Some(ref search_config) = provider.search_config {
            if search_config.query.is_empty() {
                return Err(BacklogError::ValidationError(
                    format!("Wallhaven provider '{}' has empty search query", provider_name)
                ));
            }

            if search_config.max_results == 0 || search_config.max_results > 100 {
                return Err(BacklogError::ValidationError(
                    format!("Wallhaven provider '{}' has invalid max_results: {}", 
                        provider_name, search_config.max_results)
                ));
            }

            let valid_sort_options = ["relevance", "date_added", "views", "favorites", "toplist"];
            if !valid_sort_options.contains(&search_config.sort_by.as_str()) {
                return Err(BacklogError::ValidationError(
                    format!("Wallhaven provider '{}' has invalid sort_by: {}", 
                        provider_name, search_config.sort_by)
                ));
            }

            println!("✓ Wallhaven search config validated for provider '{}'", provider_name);
        } else {
            println!("ℹ No search config for Wallhaven provider '{}'", provider_name);
        }

        Ok(())
    }

    pub fn validate_source_schemas(&self, provider_name: &str) -> Result<(), BacklogError> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| BacklogError::ValidationError(format!("Provider '{}' not found", provider_name)))?;

        if provider.source_schemas.is_empty() {
            return Err(BacklogError::ValidationError(
                format!("Provider '{}' has no source schemas defined", provider_name)
            ));
        }

        let valid_schemas = ["http", "https", "ftp", "file"];
        for schema in &provider.source_schemas {
            if !valid_schemas.contains(&schema.as_str()) {
                return Err(BacklogError::ValidationError(
                    format!("Provider '{}' has invalid schema: {}", provider_name, schema)
                ));
            }
        }

        println!("✓ Source schemas validated for provider '{}'", provider_name);
        Ok(())
    }

    pub async fn test_provider_with_retry(&self, provider_name: &str) -> Result<(), BacklogError> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| BacklogError::ValidationError(format!("Provider '{}' not found", provider_name)))?;

        let retry_policy = &provider.retry_policy;
        let mut last_error = None;

        for attempt in 0..=retry_policy.max_retries {
            if attempt > 0 {
                let backoff = std::cmp::min(
                    retry_policy.initial_backoff_ms * 2u64.pow(attempt - 1),
                    retry_policy.max_backoff_ms
                );
                println!("Retry attempt {} for provider '{}' (backoff: {}ms)", 
                    attempt, provider_name, backoff);
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }

            match self.test_provider_connection(provider_name).await {
                Ok(()) => {
                    println!("✓ Provider '{}' connection successful", provider_name);
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    println!("✗ Provider '{}' connection failed (attempt {})", 
                        provider_name, attempt);
                }
            }
        }

        Err(BacklogError::NetworkError(
            last_error.unwrap_or_else(|| reqwest::Error::from(std::io::Error::new(
                std::io::ErrorKind::Other, "All retries exhausted"
            )))
        ))
    }

    async fn test_provider_connection(&self, provider_name: &str) -> Result<(), reqwest::Error> {
        let provider = self.providers.get(provider_name)
            .ok_or_else(|| reqwest::Error::from(std::io::Error::new(
                std::io::ErrorKind::NotFound, "Provider not found"
            )))?;

        let timeout = Duration::from_secs(provider.timeout_seconds);
        let response = self.client
            .get(format!("https://{}.example.com/health", provider_name))
            .timeout(timeout)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(reqwest::Error::from(std::io::Error::new(
                std::io::ErrorKind::Other, 
                format!("Provider returned status: {}", response.status())
            )))
        }
    }

    pub async fn run_full_audit(&self) -> Result<(), BacklogError> {
        println!("Starting quality follow-up backlog audit...\n");

        for (name, provider) in &self.providers {
            if !provider.enabled {
                println!("ℹ Skipping disabled provider '{}'", name);
                continue;
            }

            println!("Processing provider '{}'...", name);
            
            // Validate Wallhaven search config
            if name.to_lowercase().contains("wallhaven") {
                self.validate_wallhaven_search_config(name)?;
            }

            // Validate source schemas
            self.validate_source_schemas(name)?;

            // Test with retry policy
            self.test_provider_with_retry(name).await?;

            println!("✓ Provider '{}' passed all checks\n", name);
        }

        println!("Audit completed successfully!");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), BacklogError> {
    let mut backlog = QualityBacklog::new();
    
    // Load provider configurations
    backlog.load_providers("config/providers.json")?;
    
    // Run the full audit
    backlog.run_full_audit().await?;
    
    Ok(())
}
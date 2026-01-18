//! Registry client module for verifying package existence.
//!
//! Provides async clients for checking if packages exist in various registries:
//! - PyPI (Python Package Index)
//! - npm (Node Package Manager)
//! - crates.io (Rust crates)
//! - Go proxy (Go modules)

mod cache;
mod crates;
mod go;
mod npm;
mod pypi;

pub use cache::RegistryCache;

use crate::contract::{DependencyVerificationConfig, RegistryConfig};
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during registry checks.
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("request timed out")]
    Timeout,
    #[error("rate limited by registry")]
    RateLimited,
    #[error("registry unavailable: {0}")]
    Unavailable(String),
}

/// Result of checking if a package exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageStatus {
    /// Package exists in the registry
    Exists,
    /// Package does not exist (404)
    NotFound,
    /// Could not determine (timeout, error, etc.)
    Unknown(String),
}

/// The type of registry to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryType {
    PyPI,
    Npm,
    Crates,
    Go,
}

impl RegistryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RegistryType::PyPI => "pypi",
            RegistryType::Npm => "npm",
            RegistryType::Crates => "crates",
            RegistryType::Go => "go",
        }
    }

    /// Get the file extensions associated with this registry.
    pub fn extensions(&self) -> &[&'static str] {
        match self {
            RegistryType::PyPI => &["py"],
            RegistryType::Npm => &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            RegistryType::Crates => &["rs"],
            RegistryType::Go => &["go"],
        }
    }

    /// Determine registry type from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "py" => Some(RegistryType::PyPI),
            "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" => Some(RegistryType::Npm),
            "rs" => Some(RegistryType::Crates),
            "go" => Some(RegistryType::Go),
            _ => None,
        }
    }
}

impl std::fmt::Display for RegistryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Registry client that can check multiple registries.
pub struct RegistryClient {
    http: reqwest::Client,
    cache: RegistryCache,
    config: DependencyVerificationConfig,
}

impl RegistryClient {
    /// Create a new registry client with the given configuration.
    pub fn new(config: DependencyVerificationConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("hollowcheck/0.1.0")
            .build()
            .expect("failed to create HTTP client");

        let cache = RegistryCache::new(config.cache_ttl_hours);

        Self {
            http,
            cache,
            config,
        }
    }

    /// Check if a package exists in the specified registry.
    pub async fn check_package(
        &self,
        registry: RegistryType,
        package: &str,
    ) -> Result<PackageStatus, RegistryError> {
        // Check if this registry is enabled
        let reg_config = self.get_registry_config(registry);
        if !reg_config.enabled {
            return Ok(PackageStatus::Unknown("registry disabled".to_string()));
        }

        // Check cache first
        if let Some(cached) = self.cache.get(registry, package) {
            return Ok(cached);
        }

        // Make the request
        let timeout = Duration::from_millis(reg_config.timeout_ms);
        let status = match registry {
            RegistryType::PyPI => pypi::check(&self.http, package, timeout).await,
            RegistryType::Npm => npm::check(&self.http, package, timeout).await,
            RegistryType::Crates => crates::check(&self.http, package, timeout).await,
            RegistryType::Go => go::check(&self.http, package, timeout).await,
        };

        // Cache the result (both positive and negative)
        match &status {
            Ok(PackageStatus::Exists) | Ok(PackageStatus::NotFound) => {
                self.cache
                    .set(registry, package, status.as_ref().unwrap().clone());
            }
            _ => {}
        }

        status
    }

    /// Get the configuration for a specific registry.
    fn get_registry_config(&self, registry: RegistryType) -> &RegistryConfig {
        match registry {
            RegistryType::PyPI => &self.config.registries.pypi,
            RegistryType::Npm => &self.config.registries.npm,
            RegistryType::Crates => &self.config.registries.crates,
            RegistryType::Go => &self.config.registries.go,
        }
    }

    /// Check if a package is in the allowlist.
    pub fn is_allowlisted(&self, package: &str) -> bool {
        use globset::{Glob, GlobSetBuilder};

        if self.config.allowlist.is_empty() {
            return false;
        }

        // Build a glob set from the allowlist patterns
        let mut builder = GlobSetBuilder::new();
        for pattern in &self.config.allowlist {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            } else if pattern == package {
                // Exact match fallback for invalid glob patterns
                return true;
            }
        }

        if let Ok(glob_set) = builder.build() {
            glob_set.is_match(package)
        } else {
            // Fallback to exact matching
            self.config.allowlist.iter().any(|p| p == package)
        }
    }

    /// Whether to fail on timeout errors.
    pub fn fail_on_timeout(&self) -> bool {
        self.config.fail_on_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_type_from_extension() {
        assert_eq!(RegistryType::from_extension("py"), Some(RegistryType::PyPI));
        assert_eq!(RegistryType::from_extension("js"), Some(RegistryType::Npm));
        assert_eq!(RegistryType::from_extension("ts"), Some(RegistryType::Npm));
        assert_eq!(
            RegistryType::from_extension("rs"),
            Some(RegistryType::Crates)
        );
        assert_eq!(RegistryType::from_extension("go"), Some(RegistryType::Go));
        assert_eq!(RegistryType::from_extension("java"), None);
    }

    #[test]
    fn test_allowlist_exact_match() {
        let config = DependencyVerificationConfig {
            enabled: true,
            allowlist: vec!["my-internal-pkg".to_string()],
            ..Default::default()
        };
        let client = RegistryClient::new(config);

        assert!(client.is_allowlisted("my-internal-pkg"));
        assert!(!client.is_allowlisted("other-pkg"));
    }

    #[test]
    fn test_allowlist_glob_pattern() {
        let config = DependencyVerificationConfig {
            enabled: true,
            allowlist: vec!["company-*".to_string(), "@myorg/*".to_string()],
            ..Default::default()
        };
        let client = RegistryClient::new(config);

        assert!(client.is_allowlisted("company-utils"));
        assert!(client.is_allowlisted("company-core"));
        assert!(client.is_allowlisted("@myorg/auth"));
        assert!(!client.is_allowlisted("other-pkg"));
    }
}

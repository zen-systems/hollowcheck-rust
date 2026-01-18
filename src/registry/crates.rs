//! crates.io (Rust crates) registry client.
//!
//! Checks package existence via: GET https://crates.io/api/v1/crates/{crate}

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Check if a crate exists on crates.io.
pub async fn check(
    client: &Client,
    crate_name: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    // Normalize crate name (crates.io allows both - and _, treats them the same)
    let normalized = normalize_crate_name(crate_name);
    let url = format!("https://crates.io/api/v1/crates/{}", normalized);

    let response = client
        .get(&url)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                RegistryError::Timeout
            } else {
                RegistryError::Network(e)
            }
        })?;

    match response.status().as_u16() {
        200 => Ok(PackageStatus::Exists),
        404 => Ok(PackageStatus::NotFound),
        429 => Err(RegistryError::RateLimited),
        status => Ok(PackageStatus::Unknown(format!("HTTP {}", status))),
    }
}

/// Normalize a crate name.
/// crates.io treats - and _ as equivalent, but the canonical form uses -.
fn normalize_crate_name(name: &str) -> String {
    name.replace('_', "-").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_crate_name() {
        assert_eq!(normalize_crate_name("serde"), "serde");
        assert_eq!(normalize_crate_name("serde_json"), "serde-json");
        assert_eq!(normalize_crate_name("Tokio"), "tokio");
        assert_eq!(normalize_crate_name("tree_sitter"), "tree-sitter");
    }
}

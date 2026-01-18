//! Go module proxy registry client.
//!
//! Checks package existence via: GET https://proxy.golang.org/{module}/@v/list
//! Returns 200 with version list if module exists, 404 if not.

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Check if a Go module exists.
pub async fn check(
    client: &Client,
    module: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    // Go modules use case-sensitive paths but proxy requires lowercase encoding
    // for uppercase letters (e.g., GitHub -> !github)
    let encoded = encode_module_path(module);
    let url = format!("https://proxy.golang.org/{}/@v/list", encoded);

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
        404 | 410 => Ok(PackageStatus::NotFound), // 410 Gone for retracted modules
        429 => Err(RegistryError::RateLimited),
        status => Ok(PackageStatus::Unknown(format!("HTTP {}", status))),
    }
}

/// Encode a Go module path for the proxy.
/// Uppercase letters are encoded as !lowercase (e.g., GitHub -> !github).
fn encode_module_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len() * 2);

    for c in path.chars() {
        if c.is_ascii_uppercase() {
            result.push('!');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_module_path() {
        assert_eq!(encode_module_path("github.com/user/repo"), "github.com/user/repo");
        assert_eq!(
            encode_module_path("github.com/Azure/azure-sdk-for-go"),
            "github.com/!azure/azure-sdk-for-go"
        );
        assert_eq!(
            encode_module_path("github.com/BurntSushi/toml"),
            "github.com/!burnt!sushi/toml"
        );
    }
}

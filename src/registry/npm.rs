//! npm (Node Package Manager) registry client.
//!
//! Checks package existence via: GET https://registry.npmjs.org/{package}
//! Handles scoped packages like @org/package

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Check if a package exists on npm.
pub async fn check(
    client: &Client,
    package: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    // URL encode the package name (important for scoped packages like @types/node)
    let encoded = encode_package_name(package);
    let url = format!("https://registry.npmjs.org/{}", encoded);

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

/// URL encode a package name for npm registry.
/// Scoped packages like @org/package need the @ and / encoded.
fn encode_package_name(name: &str) -> String {
    // For scoped packages, we need to encode the whole thing
    if name.starts_with('@') {
        // URL encode: @ -> %40, / -> %2f
        name.replace('@', "%40").replace('/', "%2f")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_regular_package() {
        assert_eq!(encode_package_name("lodash"), "lodash");
        assert_eq!(encode_package_name("express"), "express");
    }

    #[test]
    fn test_encode_scoped_package() {
        assert_eq!(encode_package_name("@types/node"), "%40types%2fnode");
        assert_eq!(encode_package_name("@babel/core"), "%40babel%2fcore");
    }
}

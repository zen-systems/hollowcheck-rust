//! PyPI (Python Package Index) registry client.
//!
//! Checks package existence via: GET https://pypi.org/pypi/{package}/json

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Check if a package exists on PyPI.
pub async fn check(
    client: &Client,
    package: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    // Normalize package name (PEP 503: lowercase, replace _ with -)
    let normalized = normalize_package_name(package);
    let url = format!("https://pypi.org/pypi/{}/json", normalized);

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

/// Normalize a Python package name per PEP 503.
/// - Lowercase
/// - Replace consecutive runs of [-_.] with a single -
fn normalize_package_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut prev_separator = false;

    for c in name.chars() {
        match c {
            '-' | '_' | '.' => {
                if !prev_separator {
                    result.push('-');
                    prev_separator = true;
                }
            }
            c => {
                result.push(c.to_ascii_lowercase());
                prev_separator = false;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_package_name() {
        assert_eq!(normalize_package_name("Requests"), "requests");
        assert_eq!(normalize_package_name("Flask_RESTful"), "flask-restful");
        assert_eq!(normalize_package_name("a__b--c..d"), "a-b-c-d");
        assert_eq!(
            normalize_package_name("typing_extensions"),
            "typing-extensions"
        );
    }
}

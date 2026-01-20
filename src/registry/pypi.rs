//! PyPI (Python Package Index) registry client.
//!
//! Checks package existence via: GET https://pypi.org/pypi/{package}/json

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Common Python import names that differ from their PyPI package names.
/// Maps: import_name -> pypi_package_name
fn get_package_alias(import_name: &str) -> Option<&'static str> {
    match import_name.to_lowercase().as_str() {
        "yaml" => Some("pyyaml"),
        "mysqldb" => Some("mysqlclient"),
        "cv2" => Some("opencv-python"),
        "pil" => Some("pillow"),
        "sklearn" => Some("scikit-learn"),
        "bs4" => Some("beautifulsoup4"),
        "dateutil" => Some("python-dateutil"),
        "dotenv" => Some("python-dotenv"),
        "jwt" => Some("pyjwt"),
        "magic" => Some("python-magic"),
        "usb" => Some("pyusb"),
        "serial" => Some("pyserial"),
        "wx" => Some("wxpython"),
        "gi" => Some("pygobject"),
        "cairo" => Some("pycairo"),
        _ => None,
    }
}

/// Check if a package exists on PyPI.
pub async fn check(
    client: &Client,
    package: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    // First check if this is a known alias
    let actual_package = get_package_alias(package).unwrap_or(package);

    // Normalize package name (PEP 503: lowercase, replace _ with -)
    let normalized = normalize_package_name(actual_package);
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

//! PyPI (Python Package Index) registry client.
//!
//! Uses a smart variant-based approach to handle Python's import-name vs package-name mismatch.
//! Instead of maintaining an unmaintainable alias list, we try multiple common naming patterns.

use super::{PackageStatus, RegistryError};
use reqwest::Client;
use std::time::Duration;

/// Check if a package exists on PyPI using smart variant matching.
///
/// Python's import names often differ from PyPI package names. Instead of maintaining
/// a brittle alias list, we try multiple common naming patterns:
///
/// 1. Direct normalized name (PEP 503)
/// 2. With `py` prefix (mysensors → pymysensors)
/// 3. With `python-` prefix (vlc → python-vlc)
/// 4. With `-py` suffix (some-lib → some-lib-py)
/// 5. With `-client` suffix (for API packages)
/// 6. With `-api` suffix (for API packages)
/// 7. With `.py` suffix (mastodon → Mastodon.py)
///
/// Only returns NotFound if ALL variants fail - this catches true AI hallucinations
/// while avoiding false positives from naming mismatches.
pub async fn check(
    client: &Client,
    package: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    let normalized = normalize_package_name(package);
    let variants = generate_name_variants(&normalized);

    // Try each variant - return Exists on first match
    for variant in &variants {
        match check_single(client, variant, timeout).await {
            Ok(PackageStatus::Exists) => return Ok(PackageStatus::Exists),
            Ok(PackageStatus::NotFound) => continue,
            // On rate limit or network error, don't fail - benefit of the doubt
            Err(RegistryError::RateLimited) => return Ok(PackageStatus::Exists),
            Err(RegistryError::Timeout) => continue, // Try next variant
            Err(e) => return Err(e),
            Ok(PackageStatus::Unknown(_)) => continue,
        }
    }

    // None of the variants exist - likely a phantom/hallucinated package
    Ok(PackageStatus::NotFound)
}

/// Generate name variants to try on PyPI.
///
/// Handles common Python import-name vs package-name patterns without aliases.
fn generate_name_variants(normalized: &str) -> Vec<String> {
    let mut variants = Vec::with_capacity(10);

    // 1. Direct normalized name
    variants.push(normalized.to_string());

    // 2. With "py" prefix (if not already prefixed)
    if !normalized.starts_with("py") {
        variants.push(format!("py{}", normalized));
    }

    // 3. With "python-" prefix
    if !normalized.starts_with("python-") {
        variants.push(format!("python-{}", normalized));
    }

    // 4. With "-py" suffix
    if !normalized.ends_with("-py") {
        variants.push(format!("{}-py", normalized));
    }

    // 5. With "-client" suffix (common for API wrappers)
    if !normalized.ends_with("-client") {
        variants.push(format!("{}-client", normalized));
    }

    // 6. With "-api" suffix
    if !normalized.ends_with("-api") {
        variants.push(format!("{}-api", normalized));
    }

    // 7. Without common prefixes (in case import has prefix but PyPI doesn't)
    if let Some(stripped) = normalized.strip_prefix("py") {
        if !stripped.is_empty() && stripped != "thon" {
            variants.push(stripped.to_string());
        }
    }
    if let Some(stripped) = normalized.strip_prefix("python-") {
        if !stripped.is_empty() {
            variants.push(stripped.to_string());
        }
    }

    // 8. Try with ".py" suffix (Mastodon.py style)
    variants.push(format!("{}.py", normalized));

    // 9. Try with "async" suffix/prefix variations
    if normalized.contains("async") {
        let without_async = normalized.replace("async", "");
        if !without_async.is_empty() && without_async != "-" {
            variants.push(without_async.trim_matches('-').to_string());
        }
    } else {
        variants.push(format!("{}-async", normalized));
    }

    variants
}

/// Check a single package name on PyPI using HEAD request for speed.
async fn check_single(
    client: &Client,
    package_name: &str,
    timeout: Duration,
) -> Result<PackageStatus, RegistryError> {
    let url = format!("https://pypi.org/pypi/{}/json", package_name);

    // Use HEAD request - faster than GET since we only need status
    let response = client
        .head(&url)
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
                if !prev_separator && !result.is_empty() {
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

    // Trim trailing separator
    if result.ends_with('-') {
        result.pop();
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
        // Leading/trailing separators
        assert_eq!(normalize_package_name("_private"), "private");
        assert_eq!(normalize_package_name("trailing_"), "trailing");
    }

    #[test]
    fn test_generate_name_variants() {
        let variants = generate_name_variants("mysensors");
        assert!(variants.contains(&"mysensors".to_string()));
        assert!(variants.contains(&"pymysensors".to_string()));
        assert!(variants.contains(&"python-mysensors".to_string()));
        assert!(variants.contains(&"mysensors-client".to_string()));

        // Test that we don't double-prefix
        let variants = generate_name_variants("pysomething");
        assert!(variants.contains(&"pysomething".to_string()));
        // Should have "something" as a stripped variant
        assert!(variants.contains(&"something".to_string()));
    }

    #[test]
    fn test_variants_for_async_packages() {
        let variants = generate_name_variants("evohomeasync");
        // Should try without "async"
        assert!(variants.iter().any(|v| v == "evohome"), "variants: {:?}", variants);

        // For non-async names, should add -async suffix
        let variants2 = generate_name_variants("evohome");
        assert!(variants2.iter().any(|v| v == "evohome-async"), "variants: {:?}", variants2);
    }
}

// hollowcheck:ignore-file mock_data - Test fixtures contain fake IDs
//! Detection of hallucinated dependencies.
//!
//! Verifies that imported packages actually exist in their respective registries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::contract::DependencyVerificationConfig;
use crate::registry::{PackageStatus, RegistryClient, RegistryType};

use super::imports::{extract_imports, ImportedDependency};
use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Detect local package names from project manifest files.
/// Returns package names that should be auto-allowlisted.
fn detect_local_packages(base_dir: &Path) -> Vec<String> {
    let mut local_packages = Vec::new();

    // Rust: Cargo.toml
    let cargo_toml = base_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if let Some(name) = parse_cargo_package_name(&content) {
                local_packages.push(name);
            }
        }
    }

    // Node.js: package.json
    let package_json = base_dir.join("package.json");
    if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Some(name) = parse_npm_package_name(&content) {
                local_packages.push(name);
            }
        }
    }

    // Go: go.mod
    let go_mod = base_dir.join("go.mod");
    if go_mod.exists() {
        if let Ok(content) = std::fs::read_to_string(&go_mod) {
            if let Some(name) = parse_go_module_name(&content) {
                local_packages.push(name);
            }
        }
    }

    // Python: pyproject.toml
    let pyproject = base_dir.join("pyproject.toml");
    if pyproject.exists() {
        if let Ok(content) = std::fs::read_to_string(&pyproject) {
            if let Some(name) = parse_pyproject_name(&content) {
                local_packages.push(name);
            }
        }
    }

    local_packages
}

/// Parse package name from Cargo.toml
fn parse_cargo_package_name(content: &str) -> Option<String> {
    // Simple regex to find: name = "package_name"
    let re = regex::Regex::new(r#"(?m)^\s*name\s*=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse package name from package.json
fn parse_npm_package_name(content: &str) -> Option<String> {
    // Simple regex to find: "name": "package-name"
    let re = regex::Regex::new(r#""name"\s*:\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse module name from go.mod
fn parse_go_module_name(content: &str) -> Option<String> {
    // Find: module github.com/user/repo
    let re = regex::Regex::new(r"(?m)^module\s+(\S+)").ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse package name from pyproject.toml
fn parse_pyproject_name(content: &str) -> Option<String> {
    // Find: name = "package_name" in [project] or [tool.poetry] section
    let re = regex::Regex::new(r#"(?m)^\s*name\s*=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Detect hallucinated dependencies in the given files.
pub fn detect_hallucinated_dependencies(
    base_dir: &Path,
    files: &[PathBuf],
    config: Option<&DependencyVerificationConfig>,
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    // If no config or disabled, skip
    let config = match config {
        Some(c) if c.is_enabled() => c,
        _ => return Ok(result),
    };

    // Extract all imports from all files
    let mut all_imports: Vec<ImportedDependency> = Vec::new();
    for file in files {
        if let Ok(imports) = extract_imports(file) {
            all_imports.extend(imports);
        }
        result.scanned += 1;
    }

    if all_imports.is_empty() {
        return Ok(result);
    }

    // Detect local package names and add to allowlist
    let local_packages = detect_local_packages(base_dir);

    // Create registry client with extended allowlist
    let mut extended_config = config.clone();
    extended_config.allowlist.extend(local_packages);
    let client = RegistryClient::new(extended_config);

    // Deduplicate imports by (registry, name)
    let mut unique_imports: HashMap<(RegistryType, String), Vec<ImportedDependency>> =
        HashMap::new();
    for import in all_imports {
        unique_imports
            .entry((import.registry, import.name.clone()))
            .or_default()
            .push(import);
    }

    // Filter allowlisted packages before checking
    let packages_to_check: usize = unique_imports
        .keys()
        .filter(|(_, pkg)| !client.is_allowlisted(pkg))
        .count();

    // Skip if nothing to check
    if packages_to_check == 0 {
        return Ok(result);
    }

    // Check each unique package using tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;
    let violations = runtime.block_on(async { check_packages(&client, unique_imports).await });

    // Log cache stats for debugging (only if HOLLOWCHECK_DEBUG is set)
    if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
        let (hits, misses) = client.cache_stats();
        eprintln!(
            "[debug] Registry cache: {} hits, {} misses ({} unique packages checked)",
            hits, misses, packages_to_check
        );
    }

    for v in violations {
        result.add_violation(v);
    }

    Ok(result)
}

/// Check packages against registries asynchronously with concurrent requests.
/// Uses buffer_unordered for parallel requests with rate limiting.
async fn check_packages(
    client: &RegistryClient,
    imports: HashMap<(RegistryType, String), Vec<ImportedDependency>>,
) -> Vec<Violation> {
    use futures::stream::{self, StreamExt};

    // Filter out allowlisted packages first
    let packages_to_check: Vec<_> = imports
        .into_iter()
        .filter(|((_, package), _)| !client.is_allowlisted(package))
        .collect();

    if packages_to_check.is_empty() {
        return Vec::new();
    }

    // Check packages concurrently with up to 50 parallel requests
    let results: Vec<_> = stream::iter(packages_to_check)
        .map(|((registry, package), locations)| async move {
            let status = client.check_package(registry, &package).await;
            (registry, package, locations, status)
        })
        .buffer_unordered(50) // Process up to 50 packages concurrently
        .collect()
        .await;

    // Process results into violations
    let mut violations = Vec::new();
    let fail_on_timeout = client.fail_on_timeout();

    for (registry, package, locations, status) in results {
        match status {
            Ok(PackageStatus::NotFound) => {
                // Package doesn't exist - create violation for each location
                for loc in locations {
                    violations.push(Violation {
                        rule: ViolationRule::HallucinatedDependency,
                        message: format!(
                            "package \"{}\" not found in {}",
                            package,
                            registry.as_str()
                        ),
                        file: loc.file,
                        line: loc.line,
                        severity: Severity::Critical,
                    });
                }
            }
            Ok(PackageStatus::Exists) => {
                // Package exists, no violation
            }
            Ok(PackageStatus::Unknown(reason)) => {
                // Could not determine - warn if fail_on_timeout is set
                if fail_on_timeout {
                    for loc in locations {
                        violations.push(Violation {
                            rule: ViolationRule::HallucinatedDependency,
                            message: format!(
                                "could not verify \"{}\" in {}: {}",
                                package,
                                registry.as_str(),
                                reason
                            ),
                            file: loc.file,
                            line: loc.line,
                            severity: Severity::Warning,
                        });
                    }
                }
            }
            Err(e) => {
                // Network error - handle based on config
                if fail_on_timeout {
                    for loc in &locations {
                        violations.push(Violation {
                            rule: ViolationRule::HallucinatedDependency,
                            message: format!("registry error checking \"{}\": {}", package, e),
                            file: loc.file.clone(),
                            line: loc.line,
                            severity: Severity::Warning,
                        });
                    }
                }
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_skip_when_disabled() {
        let config = DependencyVerificationConfig {
            enabled: false,
            ..Default::default()
        };

        let temp = TempDir::new().unwrap();
        let file = create_test_file(
            &temp,
            "test.py",
            "import definitely_not_a_real_package_12345",
        );

        let result = detect_hallucinated_dependencies(temp.path(), &[file], Some(&config)).unwrap();
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_parse_cargo_package_name() {
        let content = r#"
[package]
name = "my-cool-crate"
version = "0.1.0"
"#;
        assert_eq!(
            parse_cargo_package_name(content),
            Some("my-cool-crate".to_string())
        );
    }

    #[test]
    fn test_parse_npm_package_name() {
        let content = r#"{"name": "@org/my-package", "version": "1.0.0"}"#;
        assert_eq!(
            parse_npm_package_name(content),
            Some("@org/my-package".to_string())
        );
    }

    #[test]
    fn test_parse_go_module_name() {
        let content = "module github.com/user/myproject\n\ngo 1.21\n";
        assert_eq!(
            parse_go_module_name(content),
            Some("github.com/user/myproject".to_string())
        );
    }

    #[test]
    fn test_parse_pyproject_name() {
        let content = r#"
[project]
name = "my-python-pkg"
version = "0.1.0"
"#;
        assert_eq!(
            parse_pyproject_name(content),
            Some("my-python-pkg".to_string())
        );
    }

    #[test]
    fn test_detect_local_packages_cargo() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"[package]
name = "local-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        let packages = detect_local_packages(temp.path());
        assert!(packages.contains(&"local-crate".to_string()));
    }

    #[test]
    fn test_allowlist_exact() {
        let config = DependencyVerificationConfig {
            enabled: true,
            allowlist: vec!["my_internal_pkg".to_string()],
            ..Default::default()
        };

        let client = RegistryClient::new(config);
        assert!(client.is_allowlisted("my_internal_pkg"));
        assert!(!client.is_allowlisted("other_pkg"));
    }

    #[test]
    fn test_allowlist_glob() {
        let config = DependencyVerificationConfig {
            enabled: true,
            allowlist: vec!["company-*".to_string()],
            ..Default::default()
        };

        let client = RegistryClient::new(config);
        assert!(client.is_allowlisted("company-utils"));
        assert!(client.is_allowlisted("company-core"));
        assert!(!client.is_allowlisted("other-pkg"));
    }
}

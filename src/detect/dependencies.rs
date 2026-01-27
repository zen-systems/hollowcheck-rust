// hollowcheck:ignore-file mock_data - Test fixtures contain fake IDs
//! Detection of hallucinated dependencies.
//!
//! Uses a trait-based architecture for manifest validation:
//!
//! 1. **Manifest Validation**: Use project manifests to validate imports.
//!    Different project types (Home Assistant, standard Python) have specific
//!    implementations of the `ManifestProvider` trait.
//!
//! 2. **PyPI Fallback**: For packages not covered by manifest, check if they
//!    exist on PyPI. Only flags packages that truly don't exist anywhere.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::contract::DependencyVerificationConfig;
use crate::registry::{PackageStatus, RegistryClient, RegistryType};

use super::imports::{extract_imports, ImportedDependency};
use super::manifest::{
    detect_manifest_type, GoManifest, HomeAssistantManifest, ManifestProvider, ManifestType,
    NoManifest, PythonManifest,
};
use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Dependency validator using the trait-based manifest system.
///
/// This struct combines manifest-based validation with PyPI fallback
/// to detect hallucinated (non-existent) dependencies.
pub struct DependencyValidator {
    /// The type of manifest being used
    manifest_type: ManifestType,
    /// The manifest provider (implements ManifestProvider trait)
    manifest: Box<dyn ManifestProvider>,
    /// Registry client for PyPI checking
    registry_client: RegistryClient,
    /// Local packages to auto-allowlist
    #[allow(dead_code)]
    local_packages: Vec<String>,
}

impl DependencyValidator {
    /// Create a new DependencyValidator with auto-detected or specified manifest type.
    pub fn new(
        manifest_type: ManifestType,
        project_root: &Path,
        config: &DependencyVerificationConfig,
    ) -> anyhow::Result<Self> {
        let detected_type = match manifest_type {
            ManifestType::Auto => detect_manifest_type(project_root),
            other => other,
        };

        let manifest: Box<dyn ManifestProvider> = match detected_type {
            ManifestType::HomeAssistant => {
                if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
                    eprintln!("[debug] Detected Home Assistant project, loading component manifests...");
                }
                Box::new(HomeAssistantManifest::from_root(project_root)?)
            }
            ManifestType::PythonStandard => {
                if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
                    eprintln!("[debug] Detected Python project, loading manifests...");
                }
                Box::new(PythonManifest::from_root(project_root)?)
            }
            ManifestType::Go => {
                if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
                    eprintln!("[debug] Detected Go project, loading go.mod...");
                }
                Box::new(GoManifest::from_root(project_root)?)
            }
            ManifestType::None | ManifestType::Auto => {
                if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
                    eprintln!("[debug] No manifest detected, using pure PyPI checking...");
                }
                Box::new(NoManifest::new())
            }
        };

        if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
            let stats = manifest.stats();
            eprintln!(
                "[debug] Loaded {} scoped manifests, {} total packages",
                stats.scoped_count, stats.package_count
            );
        }

        // Detect local packages and extend allowlist
        let local_packages = detect_local_packages(project_root);
        let mut extended_config = config.clone();
        extended_config.allowlist.extend(local_packages.clone());

        Ok(Self {
            manifest_type: detected_type,
            manifest,
            registry_client: RegistryClient::new(extended_config),
            local_packages,
        })
    }

    /// Get the detected manifest type.
    pub fn manifest_type(&self) -> &ManifestType {
        &self.manifest_type
    }

    /// Check if an import is valid according to the manifest.
    pub fn is_valid_import(&self, import_name: &str, file_path: &Path) -> bool {
        self.manifest.is_valid_import(import_name, file_path)
    }

    /// Get the scope (component/module) a file belongs to.
    pub fn get_scope(&self, file_path: &Path) -> Option<String> {
        self.manifest.get_scope(file_path)
    }

    /// Validate an import and return a violation if invalid.
    ///
    /// This checks:
    /// 1. If import is allowlisted
    /// 2. If import is valid per manifest
    ///
    /// Returns None if valid, Some(Violation) if invalid.
    pub fn validate_import(
        &self,
        import_name: &str,
        file_path: &Path,
        _line: usize,
    ) -> Option<Violation> {
        // Check allowlist
        if self.registry_client.is_allowlisted(import_name) {
            return None;
        }

        // Check manifest
        if self.manifest.is_valid_import(import_name, file_path) {
            return None;
        }

        // Import not in manifest - it will need PyPI verification
        None
    }

    /// Access the registry client for PyPI checking.
    pub fn registry_client(&self) -> &RegistryClient {
        &self.registry_client
    }
}

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
    let re = regex::Regex::new(r#"(?m)^\s*name\s*=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse package name from package.json
fn parse_npm_package_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#""name"\s*:\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse module name from go.mod
fn parse_go_module_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?m)^module\s+(\S+)").ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse package name from pyproject.toml
fn parse_pyproject_name(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*name\s*=\s*"([^"]+)""#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Detect hallucinated dependencies in the given files.
///
/// Uses a two-phase approach:
/// 1. **Manifest validation**: Validate imports against declared deps
/// 2. **PyPI fallback**: For packages not covered by manifest, check if they exist on PyPI
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

    // Create the validator
    let validator = DependencyValidator::new(ManifestType::Auto, base_dir, config)?;

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

    // Deduplicate imports by (registry, name)
    let mut unique_imports: HashMap<(RegistryType, String), Vec<ImportedDependency>> =
        HashMap::new();
    for import in all_imports {
        unique_imports
            .entry((import.registry, import.name.clone()))
            .or_default()
            .push(import);
    }

    // Filter imports: remove those covered by manifest or allowlist
    // Also collect Go violations directly (no PyPI check needed for Go)
    let mut go_violations: Vec<Violation> = Vec::new();

    let imports_to_check: HashMap<(RegistryType, String), Vec<ImportedDependency>> = unique_imports
        .into_iter()
        .filter(|((registry, pkg), locations)| {
            // Skip if allowlisted (works for all languages)
            if validator.registry_client().is_allowlisted(pkg) {
                return false;
            }

            // Check manifest validation (works for Python AND Go)
            if let Some(loc) = locations.first() {
                if validator.is_valid_import(pkg, Path::new(&loc.file)) {
                    return false;
                }
            }

            // For Go, if not in manifest and not in allowlist, it's a violation
            // Go doesn't need registry checking - go.mod is authoritative
            if *registry == RegistryType::Go {
                for loc in locations {
                    go_violations.push(Violation {
                        rule: ViolationRule::HallucinatedDependency,
                        message: format!(
                            "Go import \"{}\" not found in go.mod",
                            pkg
                        ),
                        file: loc.file.clone(),
                        line: loc.line,
                        severity: Severity::Critical,
                    });
                }
                return false; // Don't include in PyPI check
            }

            // For Python (and other registries), need registry verification
            true
        })
        .collect();

    // Add Go violations to result
    for v in go_violations {
        result.add_violation(v);
    }

    let packages_to_check = imports_to_check.len();

    // Skip if nothing to check
    if packages_to_check == 0 {
        return Ok(result);
    }

    if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
        eprintln!(
            "[debug] {} packages to check against PyPI (after manifest filtering)",
            packages_to_check
        );
    }

    // Phase 2: Check remaining packages against PyPI
    let runtime = tokio::runtime::Runtime::new()?;
    let violations =
        runtime.block_on(async { check_packages(validator.registry_client(), imports_to_check).await });

    // Log cache stats for debugging
    if std::env::var("HOLLOWCHECK_DEBUG").is_ok() {
        let (hits, misses) = validator.registry_client().cache_stats();
        eprintln!("[debug] Registry cache: {} hits, {} misses", hits, misses);
    }

    for v in violations {
        result.add_violation(v);
    }

    Ok(result)
}

/// Check packages against registries asynchronously with concurrent requests.
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
        .buffer_unordered(50)
        .collect()
        .await;

    // Process results into violations
    let mut violations = Vec::new();
    let fail_on_timeout = client.fail_on_timeout();

    for (registry, package, locations, status) in results {
        match status {
            Ok(PackageStatus::NotFound) => {
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

        let result =
            detect_hallucinated_dependencies(temp.path(), &[file], Some(&config)).unwrap();
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

    #[test]
    fn test_dependency_validator_auto_detect_python() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("requirements.txt"),
            "requests>=2.0\nflask\n",
        )
        .unwrap();

        let config = DependencyVerificationConfig {
            enabled: true,
            ..Default::default()
        };

        let validator =
            DependencyValidator::new(ManifestType::Auto, temp.path(), &config).unwrap();
        assert_eq!(validator.manifest_type(), &ManifestType::PythonStandard);
    }

    #[test]
    fn test_dependency_validator_auto_detect_ha() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("homeassistant/components")).unwrap();

        let config = DependencyVerificationConfig {
            enabled: true,
            ..Default::default()
        };

        let validator =
            DependencyValidator::new(ManifestType::Auto, temp.path(), &config).unwrap();
        assert_eq!(validator.manifest_type(), &ManifestType::HomeAssistant);
    }

    #[test]
    fn test_dependency_validator_validates_imports() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("requirements.txt"),
            "pyswitchbot>=0.40.0\n",
        )
        .unwrap();

        let config = DependencyVerificationConfig {
            enabled: true,
            ..Default::default()
        };

        let validator =
            DependencyValidator::new(ManifestType::Auto, temp.path(), &config).unwrap();
        let file = temp.path().join("test.py");

        // Direct match
        assert!(validator.is_valid_import("pyswitchbot", &file));
        // Via py-prefix stripping
        assert!(validator.is_valid_import("switchbot", &file));
        // Not declared
        assert!(!validator.is_valid_import("nonexistent", &file));
    }
}

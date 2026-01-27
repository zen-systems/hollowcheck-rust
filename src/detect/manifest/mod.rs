//! Trait-based manifest validation system.
//!
//! This module provides an extensible architecture for validating imports
//! against project manifests. Different project types (Home Assistant, standard
//! Python, etc.) can implement the `ManifestProvider` trait to provide
//! project-specific validation logic.
//!
//! # Architecture
//!
//! ```text
//! ManifestProvider trait
//!     ├── HomeAssistantManifest  (component-scoped manifests with loggers)
//!     ├── PythonManifest         (pyproject.toml, requirements.txt, setup.cfg)
//!     └── NoManifest             (pure PyPI phantom detection)
//! ```

use std::path::Path;

mod golang;
mod homeassistant;
mod none;
mod python;

pub use golang::GoManifest;
pub use homeassistant::{ComponentData, HomeAssistantManifest};
pub use none::NoManifest;
pub use python::PythonManifest;

/// Manifest type for dependency validation.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ManifestType {
    /// Home Assistant project with component manifest.json files
    HomeAssistant,
    /// Standard Python project (pyproject.toml, requirements.txt, etc.)
    PythonStandard,
    /// Go project with go.mod
    Go,
    /// Auto-detect based on directory structure
    #[default]
    Auto,
    /// No manifest validation, pure PyPI phantom detection
    None,
}

/// Trait for manifest-based import validation.
///
/// Implementations of this trait provide project-specific logic for
/// determining whether an import is valid for a given file.
pub trait ManifestProvider: Send + Sync {
    /// Returns true if this import is declared/valid for the given file.
    ///
    /// The implementation should check whether `import_name` is a valid
    /// dependency for the file at `file_path`, considering project-specific
    /// scoping rules (e.g., Home Assistant component manifests).
    fn is_valid_import(&self, import_name: &str, file_path: &Path) -> bool;

    /// Get declared imports for debugging/reporting.
    ///
    /// Returns all imports that are declared as valid for the given file,
    /// useful for debugging and generating detailed violation reports.
    fn get_declared_imports(&self, file_path: &Path) -> Vec<String>;

    /// Get the scope/component/module this file belongs to.
    ///
    /// Returns an optional scope identifier (e.g., component name for Home
    /// Assistant, package name for standard Python projects).
    fn get_scope(&self, file_path: &Path) -> Option<String>;

    /// Get statistics about the manifest for debugging.
    fn stats(&self) -> ManifestStats {
        ManifestStats::default()
    }
}

/// Statistics about a loaded manifest.
#[derive(Debug, Clone, Default)]
pub struct ManifestStats {
    /// Number of scoped manifests (e.g., component manifests)
    pub scoped_count: usize,
    /// Total number of declared packages
    pub package_count: usize,
}

/// Auto-detect manifest type from a directory.
pub fn detect_manifest_type(dir: &Path) -> ManifestType {
    // Check for Home Assistant structure
    if dir.join("homeassistant").join("components").exists() {
        return ManifestType::HomeAssistant;
    }

    // Check for Python project markers
    let has_python = dir.join("pyproject.toml").exists()
        || dir.join("requirements.txt").exists()
        || dir.join("setup.py").exists()
        || dir.join("setup.cfg").exists();

    if has_python {
        return ManifestType::PythonStandard;
    }

    // Check for Go project
    if dir.join("go.mod").exists() {
        return ManifestType::Go;
    }

    ManifestType::None
}

/// Common utility functions for manifest parsing.
pub mod utils {
    /// Extract package name from a requirement string.
    ///
    /// Handles various requirement formats:
    /// - `pyswitchbot==0.40.0` → `pyswitchbot`
    /// - `aiohttp>=3.0,<4` → `aiohttp`
    /// - `package[extra]>=1.0` → `package`
    /// - `tuya-device-sharing-sdk==0.2.1` → `tuya-device-sharing-sdk`
    pub fn extract_package_name(req: &str) -> String {
        req.split(|c| c == '=' || c == '>' || c == '<' || c == '~' || c == '[' || c == ';' || c == ' ')
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Normalize a package name for comparison.
    ///
    /// Converts to lowercase and normalizes separators (hyphens/underscores).
    #[allow(dead_code)]
    pub fn normalize_package_name(name: &str) -> String {
        name.to_lowercase().replace('-', "_")
    }

    /// Check if an import name matches a package name using common Python patterns.
    ///
    /// This handles the various ways Python import names differ from package names:
    /// - py prefix: `pyswitchbot` → import `switchbot`
    /// - python- prefix: `python-miio` → import `miio`
    /// - Namespace packages: `jaraco.abode` → import `jaraco`
    /// - .py suffix: `Mastodon.py` → import `mastodon`
    /// - Hyphens vs underscores: `flask-restful` → import `flask_restful`
    pub fn import_matches_package(import_name: &str, package_name: &str) -> bool {
        let pkg = package_name.to_lowercase();
        let imp = import_name.to_lowercase();

        // Direct match
        if pkg == imp {
            return true;
        }

        // Normalize: replace hyphens/underscores
        let pkg_normalized = pkg.replace('-', "_");
        let imp_normalized = imp.replace('-', "_");
        if pkg_normalized == imp_normalized {
            return true;
        }

        // py prefix: pyswitchbot → switchbot
        if let Some(stripped) = pkg.strip_prefix("py") {
            if stripped == imp || stripped.replace('-', "_") == imp_normalized {
                return true;
            }
        }

        // python- prefix: python-miio → miio
        if let Some(stripped) = pkg.strip_prefix("python-") {
            if stripped == imp || stripped.replace('-', "_") == imp_normalized {
                return true;
            }
        }

        // Namespace packages: jaraco.abode → jaraco
        if pkg.starts_with(&format!("{}.", imp)) || pkg.starts_with(&format!("{}-", imp)) {
            return true;
        }

        // -py suffix: somelib-py → somelib
        if let Some(stripped) = pkg.strip_suffix("-py") {
            if stripped == imp {
                return true;
            }
        }

        // .py suffix: Mastodon.py → mastodon
        if let Some(stripped) = pkg.strip_suffix(".py") {
            if stripped.to_lowercase() == imp {
                return true;
            }
        }

        // -client suffix: some-client → some (for API client packages)
        if let Some(stripped) = pkg.strip_suffix("-client") {
            if stripped == imp {
                return true;
            }
        }

        // -api suffix
        if let Some(stripped) = pkg.strip_suffix("-api") {
            if stripped == imp {
                return true;
            }
        }

        // Async variations: evohome-async → evohomeasync
        let pkg_no_sep = pkg.replace('-', "").replace('_', "");
        let imp_no_sep = imp.replace('-', "").replace('_', "");
        if pkg_no_sep == imp_no_sep {
            return true;
        }

        false
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_extract_package_name() {
            assert_eq!(extract_package_name("requests>=2.0"), "requests");
            assert_eq!(extract_package_name("pyswitchbot==0.40.0"), "pyswitchbot");
            assert_eq!(extract_package_name("aiohttp>=3.0,<4"), "aiohttp");
            assert_eq!(extract_package_name("package[extra]>=1.0"), "package");
            assert_eq!(extract_package_name("simple"), "simple");
            assert_eq!(
                extract_package_name("tuya-device-sharing-sdk==0.2.1"),
                "tuya-device-sharing-sdk"
            );
        }

        #[test]
        fn test_import_matches_package() {
            // Direct match
            assert!(import_matches_package("requests", "requests"));

            // py prefix
            assert!(import_matches_package("switchbot", "pyswitchbot"));
            assert!(import_matches_package("rfxtrx", "pyrfxtrx"));

            // python- prefix
            assert!(import_matches_package("miio", "python-miio"));
            assert!(import_matches_package("vlc", "python-vlc"));

            // Namespace packages
            assert!(import_matches_package("jaraco", "jaraco.abode"));
            assert!(import_matches_package("jaraco", "jaraco-abode"));

            // .py suffix
            assert!(import_matches_package("mastodon", "Mastodon.py"));

            // Hyphens vs underscores
            assert!(import_matches_package("flask_restful", "flask-restful"));
            assert!(import_matches_package("some_package", "some-package"));

            // Async variations
            assert!(import_matches_package("evohomeasync", "evohome-async"));

            // Non-matches
            assert!(!import_matches_package("notrelated", "something-else"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_manifest_type_home_assistant() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("homeassistant/components")).unwrap();
        assert_eq!(detect_manifest_type(temp.path()), ManifestType::HomeAssistant);
    }

    #[test]
    fn test_detect_manifest_type_python() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("pyproject.toml"), "[project]").unwrap();
        assert_eq!(detect_manifest_type(temp.path()), ManifestType::PythonStandard);
    }

    #[test]
    fn test_detect_manifest_type_none() {
        let temp = TempDir::new().unwrap();
        assert_eq!(detect_manifest_type(temp.path()), ManifestType::None);
    }

    #[test]
    fn test_detect_manifest_type_go() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("go.mod"), "module example.com/test\n\ngo 1.21\n").unwrap();
        assert_eq!(detect_manifest_type(temp.path()), ManifestType::Go);
    }
}

//! No-manifest provider for pure phantom detection.
//!
//! This implementation doesn't validate against any project manifest.
//! All imports are considered "not valid" from a manifest perspective,
//! meaning they will be checked against PyPI for existence.

use std::path::Path;

use super::{ManifestProvider, ManifestStats};

/// No-manifest provider.
///
/// Returns false for all imports, indicating they should be validated
/// against PyPI or other package registries. Use this when a project
/// has no manifest or when you want pure phantom detection.
pub struct NoManifest;

impl NoManifest {
    /// Create a new NoManifest provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoManifest {
    fn default() -> Self {
        Self::new()
    }
}

impl ManifestProvider for NoManifest {
    fn is_valid_import(&self, _import_name: &str, _file_path: &Path) -> bool {
        // All imports need to be validated against PyPI
        false
    }

    fn get_declared_imports(&self, _file_path: &Path) -> Vec<String> {
        // No declared imports
        Vec::new()
    }

    fn get_scope(&self, _file_path: &Path) -> Option<String> {
        // No scope information
        None
    }

    fn stats(&self) -> ManifestStats {
        ManifestStats::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_no_manifest_rejects_all() {
        let manifest = NoManifest::new();
        let path = PathBuf::from("/some/file.py");

        assert!(!manifest.is_valid_import("requests", &path));
        assert!(!manifest.is_valid_import("numpy", &path));
        assert!(!manifest.is_valid_import("any_package", &path));
    }

    #[test]
    fn test_no_manifest_empty_declared() {
        let manifest = NoManifest::new();
        let path = PathBuf::from("/some/file.py");

        assert!(manifest.get_declared_imports(&path).is_empty());
    }

    #[test]
    fn test_no_manifest_no_scope() {
        let manifest = NoManifest::new();
        let path = PathBuf::from("/some/file.py");

        assert!(manifest.get_scope(&path).is_none());
    }
}

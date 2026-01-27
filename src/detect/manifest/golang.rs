//! Go module manifest provider.
//!
//! Parses go.mod files to validate Go imports against declared dependencies.
//! Go has a unique import model where you import subpackages but only declare
//! root modules in go.mod.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{ManifestProvider, ManifestStats};

/// Go module manifest provider.
///
/// Parses go.mod files and validates imports against:
/// - The root module (internal imports)
/// - Required external modules
/// - Replace directives (local dependencies)
pub struct GoManifest {
    /// Root module path (e.g., "k8s.io/kubernetes")
    root_module: String,
    /// External module dependencies: module path → version
    external_modules: HashMap<String, String>,
    /// Replace directives: module path → local path or replacement module
    replace_directives: HashMap<String, String>,
}

impl GoManifest {
    /// Parse a go.mod file and create a GoManifest.
    pub fn from_go_mod(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse_go_mod(&content)
    }

    /// Create a GoManifest by finding go.mod in the project root.
    pub fn from_root(root: &Path) -> anyhow::Result<Self> {
        let go_mod_path = root.join("go.mod");
        if !go_mod_path.exists() {
            anyhow::bail!("go.mod not found at {:?}", go_mod_path);
        }
        Self::from_go_mod(&go_mod_path)
    }

    /// Parse go.mod content.
    fn parse_go_mod(content: &str) -> anyhow::Result<Self> {
        let mut root_module = String::new();
        let mut external_modules = HashMap::new();
        let mut replace_directives = HashMap::new();

        let mut in_require_block = false;
        let mut in_replace_block = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            // Parse module declaration
            if line.starts_with("module ") {
                root_module = line
                    .strip_prefix("module ")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                continue;
            }

            // Track block starts/ends
            if line == "require (" {
                in_require_block = true;
                continue;
            }
            if line == "replace (" {
                in_replace_block = true;
                continue;
            }
            if line == ")" {
                in_require_block = false;
                in_replace_block = false;
                continue;
            }

            // Parse single-line require
            if line.starts_with("require ") && !line.contains('(') {
                if let Some((module, version)) = parse_require_line(line.strip_prefix("require ").unwrap_or("")) {
                    external_modules.insert(module, version);
                }
                continue;
            }

            // Parse single-line replace
            if line.starts_with("replace ") && !line.contains('(') {
                if let Some((from, to)) = parse_replace_line(line.strip_prefix("replace ").unwrap_or("")) {
                    replace_directives.insert(from, to);
                }
                continue;
            }

            // Parse require block entries
            if in_require_block {
                if let Some((module, version)) = parse_require_line(line) {
                    external_modules.insert(module, version);
                }
                continue;
            }

            // Parse replace block entries
            if in_replace_block {
                if let Some((from, to)) = parse_replace_line(line) {
                    replace_directives.insert(from, to);
                }
                continue;
            }
        }

        if root_module.is_empty() {
            anyhow::bail!("No module declaration found in go.mod");
        }

        Ok(Self {
            root_module,
            external_modules,
            replace_directives,
        })
    }

    /// Check if an import path is a Go stdlib package.
    ///
    /// Go stdlib packages don't have dots in their first path component.
    /// Examples: "fmt", "net/http", "encoding/json"
    fn is_stdlib(import_path: &str) -> bool {
        let first_component = import_path.split('/').next().unwrap_or("");
        !first_component.contains('.')
    }

    /// Get the root module path.
    pub fn root_module(&self) -> &str {
        &self.root_module
    }

    /// Get the external modules.
    pub fn external_modules(&self) -> &HashMap<String, String> {
        &self.external_modules
    }

    /// Get the replace directives.
    pub fn replace_directives(&self) -> &HashMap<String, String> {
        &self.replace_directives
    }
}

/// Parse a require line: "google.golang.org/grpc v1.78.0"
fn parse_require_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();

    // Skip indirect dependencies marker and comments
    let line = line.split("//").next().unwrap_or("").trim();

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else if parts.len() == 1 && !parts[0].is_empty() {
        // Module without version (rare but possible)
        Some((parts[0].to_string(), String::new()))
    } else {
        None
    }
}

/// Parse a replace line: "k8s.io/component-base => ./staging/src/k8s.io/component-base"
fn parse_replace_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();

    // Skip comments
    let line = line.split("//").next().unwrap_or("").trim();

    // Split on "=>"
    let parts: Vec<&str> = line.split("=>").collect();
    if parts.len() == 2 {
        let from = parts[0].trim().split_whitespace().next().unwrap_or("").to_string();
        let to = parts[1].trim().split_whitespace().next().unwrap_or("").to_string();
        if !from.is_empty() && !to.is_empty() {
            return Some((from, to));
        }
    }

    None
}

impl ManifestProvider for GoManifest {
    fn is_valid_import(&self, import_name: &str, _file_path: &Path) -> bool {
        // Case 1: Go stdlib (no dots in first component)
        if Self::is_stdlib(import_name) {
            return true;
        }

        // Case 2: Internal to root module
        // "k8s.io/kubernetes/pkg/..." is internal
        if import_name == self.root_module
            || import_name.starts_with(&format!("{}/", self.root_module))
        {
            return true;
        }

        // Case 3: Replace directives (local modules)
        // "k8s.io/component-base/config" is valid if "k8s.io/component-base" is replaced
        for replaced_module in self.replace_directives.keys() {
            if import_name == replaced_module
                || import_name.starts_with(&format!("{}/", replaced_module))
            {
                return true;
            }
        }

        // Case 4: External module subpackages
        // "google.golang.org/grpc/credentials" is valid if "google.golang.org/grpc" is required
        for module in self.external_modules.keys() {
            if import_name == module || import_name.starts_with(&format!("{}/", module)) {
                return true;
            }
        }

        false
    }

    fn get_declared_imports(&self, _file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        // Add root module
        imports.push(self.root_module.clone());

        // Add external modules
        imports.extend(self.external_modules.keys().cloned());

        // Add replaced modules
        imports.extend(self.replace_directives.keys().cloned());

        imports
    }

    fn get_scope(&self, _file_path: &Path) -> Option<String> {
        Some(self.root_module.clone())
    }

    fn stats(&self) -> ManifestStats {
        ManifestStats {
            scoped_count: 0,
            package_count: self.external_modules.len() + self.replace_directives.len() + 1, // +1 for root module
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_go_mod() -> &'static str {
        r#"
module k8s.io/kubernetes

go 1.22.0

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible
	github.com/aws/aws-sdk-go v1.50.32
	google.golang.org/grpc v1.78.0
	k8s.io/api v0.0.0
	k8s.io/apimachinery v0.0.0
)

require (
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/spf13/pflag v1.0.5 // indirect
)

replace (
	k8s.io/api => ./staging/src/k8s.io/api
	k8s.io/apimachinery => ./staging/src/k8s.io/apimachinery
	k8s.io/component-base => ./staging/src/k8s.io/component-base
)
"#
    }

    #[test]
    fn test_parse_go_mod() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();

        assert_eq!(manifest.root_module, "k8s.io/kubernetes");
        assert!(manifest.external_modules.contains_key("google.golang.org/grpc"));
        assert!(manifest.external_modules.contains_key("github.com/aws/aws-sdk-go"));
        assert!(manifest.replace_directives.contains_key("k8s.io/api"));
        assert!(manifest.replace_directives.contains_key("k8s.io/component-base"));
    }

    #[test]
    fn test_stdlib_imports() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // Standard library imports (no dots in first component)
        assert!(manifest.is_valid_import("fmt", &file));
        assert!(manifest.is_valid_import("net/http", &file));
        assert!(manifest.is_valid_import("encoding/json", &file));
        assert!(manifest.is_valid_import("context", &file));
    }

    #[test]
    fn test_internal_imports() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // Internal to root module
        assert!(manifest.is_valid_import("k8s.io/kubernetes", &file));
        assert!(manifest.is_valid_import("k8s.io/kubernetes/pkg/api", &file));
        assert!(manifest.is_valid_import("k8s.io/kubernetes/cmd/kube-apiserver", &file));
    }

    #[test]
    fn test_replaced_module_imports() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // Replaced modules (local dependencies)
        assert!(manifest.is_valid_import("k8s.io/api", &file));
        assert!(manifest.is_valid_import("k8s.io/api/core/v1", &file));
        assert!(manifest.is_valid_import("k8s.io/apimachinery", &file));
        assert!(manifest.is_valid_import("k8s.io/apimachinery/pkg/types", &file));
        assert!(manifest.is_valid_import("k8s.io/component-base", &file));
        assert!(manifest.is_valid_import("k8s.io/component-base/config", &file));
    }

    #[test]
    fn test_external_module_subpackages() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // External module and subpackages
        assert!(manifest.is_valid_import("google.golang.org/grpc", &file));
        assert!(manifest.is_valid_import("google.golang.org/grpc/credentials", &file));
        assert!(manifest.is_valid_import("google.golang.org/grpc/metadata", &file));
        assert!(manifest.is_valid_import("github.com/aws/aws-sdk-go", &file));
        assert!(manifest.is_valid_import("github.com/aws/aws-sdk-go/aws", &file));
        assert!(manifest.is_valid_import("github.com/aws/aws-sdk-go/service/s3", &file));
    }

    #[test]
    fn test_indirect_dependencies() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // Indirect dependencies are still valid
        assert!(manifest.is_valid_import("github.com/spf13/pflag", &file));
    }

    #[test]
    fn test_invalid_imports() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let file = PathBuf::from("/some/file.go");

        // Non-existent modules
        assert!(!manifest.is_valid_import("github.com/nonexistent/package", &file));
        assert!(!manifest.is_valid_import("example.com/fake/module", &file));
    }

    #[test]
    fn test_from_file() {
        let temp = TempDir::new().unwrap();
        let go_mod_path = temp.path().join("go.mod");

        fs::write(&go_mod_path, sample_go_mod()).unwrap();

        let manifest = GoManifest::from_go_mod(&go_mod_path).unwrap();
        assert_eq!(manifest.root_module, "k8s.io/kubernetes");
    }

    #[test]
    fn test_single_line_require() {
        let content = r#"
module example.com/mymodule

go 1.21

require github.com/pkg/errors v0.9.1
"#;
        let manifest = GoManifest::parse_go_mod(content).unwrap();
        assert!(manifest.external_modules.contains_key("github.com/pkg/errors"));
    }

    #[test]
    fn test_single_line_replace() {
        let content = r#"
module example.com/mymodule

go 1.21

replace example.com/old => example.com/new v1.0.0
"#;
        let manifest = GoManifest::parse_go_mod(content).unwrap();
        assert!(manifest.replace_directives.contains_key("example.com/old"));
    }

    #[test]
    fn test_stats() {
        let manifest = GoManifest::parse_go_mod(sample_go_mod()).unwrap();
        let stats = manifest.stats();

        // 5 direct + 2 indirect external modules + 3 replace directives + 1 root module
        assert!(stats.package_count > 0);
    }
}

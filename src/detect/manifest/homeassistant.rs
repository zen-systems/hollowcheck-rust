//! Home Assistant manifest provider.
//!
//! Home Assistant has a unique structure with component-scoped manifests.
//! Each component in `homeassistant/components/<name>/` has a `manifest.json`
//! that declares its dependencies via `requirements` and `loggers` fields.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use phf::phf_map;
use serde::Deserialize;
use walkdir::WalkDir;

use super::utils::{extract_package_name, import_matches_package};
use super::{ManifestProvider, ManifestStats};

/// Known submodules that don't match their parent package names.
/// Maps import name → PyPI package name prefix.
static KNOWN_SUBMODULES: phf::Map<&'static str, &'static str> = phf_map! {
    "didl_lite" => "async-upnp-client",
    "inelsmqtt" => "elkoep-aio-mqtt",
};

/// Check if an import is a known submodule of a declared package.
fn is_known_submodule(import_name: &str, requirements: &[String]) -> bool {
    let import_lower = import_name.to_lowercase();
    if let Some(parent_pkg) = KNOWN_SUBMODULES.get(&import_lower) {
        return requirements.iter().any(|req| {
            let pkg = extract_package_name(req).to_lowercase();
            pkg.starts_with(parent_pkg)
        });
    }
    false
}

/// Check if an import is a Home Assistant internal package (not on PyPI).
fn is_homeassistant_internal(import_name: &str) -> bool {
    let lower = import_name.to_lowercase();
    matches!(
        lower.as_str(),
        "hass_frontend" | "homeassistant_frontend" | "insteon_frontend"
    )
}

/// Home Assistant component manifest data.
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentData {
    /// Domain name of the component
    #[serde(default)]
    pub domain: String,

    /// PyPI package requirements (e.g., "pyswitchbot==0.40.0")
    #[serde(default)]
    pub requirements: Vec<String>,

    /// Logger names used by this component (authoritative for import validation)
    #[serde(default)]
    pub loggers: Vec<String>,

    /// Other Home Assistant components this depends on
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Home Assistant manifest provider.
///
/// Parses all component manifest.json files and provides scoped validation
/// based on which component a file belongs to.
pub struct HomeAssistantManifest {
    /// Project root directory
    root: PathBuf,
    /// Component manifests indexed by component directory path
    component_manifests: HashMap<PathBuf, ComponentData>,
    /// Global requirements (from requirements*.txt at root)
    global_requirements: Vec<String>,
}

impl HomeAssistantManifest {
    /// Create a new HomeAssistantManifest by scanning the project root.
    pub fn from_root(root: &Path) -> anyhow::Result<Self> {
        let mut manifest = Self {
            root: root.to_path_buf(),
            component_manifests: HashMap::new(),
            global_requirements: Vec::new(),
        };

        // Parse component manifests
        manifest.scan_component_manifests()?;

        // Parse global requirements
        manifest.load_global_requirements()?;

        Ok(manifest)
    }

    /// Scan for and parse all component manifest.json files.
    fn scan_component_manifests(&mut self) -> anyhow::Result<()> {
        let components_dir = self.root.join("homeassistant").join("components");

        if !components_dir.exists() {
            return Ok(());
        }

        // Walk through component directories
        for entry in WalkDir::new(&components_dir)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("manifest.json")) {
                if let Some(parent) = path.parent() {
                    if let Ok(data) = self.parse_component_manifest(path) {
                        self.component_manifests.insert(parent.to_path_buf(), data);
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse a single component manifest.json file.
    fn parse_component_manifest(&self, path: &Path) -> anyhow::Result<ComponentData> {
        let content = fs::read_to_string(path)?;
        let data: ComponentData = serde_json::from_str(&content)?;
        Ok(data)
    }

    /// Load global requirements from requirements*.txt files at root.
    fn load_global_requirements(&mut self) -> anyhow::Result<()> {
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("requirements") && name.ends_with(".txt") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            for line in content.lines() {
                                let line = line.trim();
                                if !line.is_empty() && !line.starts_with('#') && !line.starts_with('-') {
                                    self.global_requirements.push(line.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Find which component a file belongs to by walking up the directory tree.
    fn find_component(&self, file_path: &Path) -> Option<&PathBuf> {
        let mut current = file_path.parent()?;

        loop {
            if self.component_manifests.contains_key(current) {
                return self.component_manifests.keys().find(|k| k.as_path() == current);
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Check if this file is in a template/scaffold directory that should be skipped.
    fn is_template_file(&self, file_path: &Path) -> bool {
        let path_str = file_path.to_string_lossy();
        path_str.contains("/script/scaffold/templates/")
            || path_str.contains("/script/hassfest/")
    }

    /// Fuzzy match an import against a list of requirements.
    ///
    /// Handles complex cases like:
    /// - `tuya-device-sharing-sdk` → import `tuya_sharing`
    /// - `py-synologydsm-api` → import `synology_dsm`
    /// - `paho-mqtt` → import `paho`
    fn fuzzy_match_requirements(&self, import_name: &str, requirements: &[String]) -> bool {
        let import_lower = import_name.to_lowercase();
        let import_normalized = import_lower.replace('-', "_");
        let import_no_sep = import_normalized.replace('_', "");

        for req in requirements {
            let pkg_name = extract_package_name(req);
            let pkg_lower = pkg_name.to_lowercase();
            let pkg_normalized = pkg_lower.replace('-', "_");
            let pkg_no_sep = pkg_normalized.replace('_', "");

            // Use the common matching logic
            if import_matches_package(import_name, &pkg_name) {
                return true;
            }

            // Match without any separators (handles underscore placement differences)
            // py-synologydsm-api → synology_dsm (synologydsm == synologydsm)
            if pkg_no_sep.contains(&import_no_sep) || import_no_sep.contains(&pkg_no_sep) {
                return true;
            }

            // Additional HA-specific matching patterns
            let pkg_parts: Vec<&str> = pkg_lower.split(|c| c == '-' || c == '_').collect();

            // Try first word + any other word matching
            // tuya-device-sharing-sdk → tuya_sharing
            if pkg_parts.len() >= 2 {
                // Try first + each subsequent word
                for i in 1..pkg_parts.len() {
                    let abbreviated = format!("{}_{}", pkg_parts[0], pkg_parts[i]);
                    if abbreviated == import_normalized {
                        return true;
                    }
                    // Without separator
                    let abbreviated_no_sep = format!("{}{}", pkg_parts[0], pkg_parts[i]);
                    if abbreviated_no_sep == import_no_sep {
                        return true;
                    }
                }
            }

            // Handle api/sdk suffix removal
            // py-synologydsm-api → synology_dsm
            let pkg_no_suffix = pkg_normalized
                .strip_suffix("_api")
                .or_else(|| pkg_normalized.strip_suffix("_sdk"))
                .or_else(|| pkg_normalized.strip_suffix("_client"))
                .unwrap_or(&pkg_normalized);

            if pkg_no_suffix == import_normalized {
                return true;
            }

            // Also strip py prefix after removing suffix
            let pkg_stripped = pkg_no_suffix.strip_prefix("py_").unwrap_or(pkg_no_suffix);
            if pkg_stripped == import_normalized {
                return true;
            }

            // Check if import without separators matches package without separators (after stripping)
            let pkg_stripped_no_sep = pkg_stripped.replace('_', "");
            if pkg_stripped_no_sep == import_no_sep {
                return true;
            }

            // Handle imports that are just the first part
            // paho-mqtt → paho
            if let Some(first_part) = pkg_lower.split(|c| c == '-' || c == '_').next() {
                if first_part == import_normalized || first_part == import_lower {
                    return true;
                }
            }
        }

        false
    }

    /// Check if import matches any loggers (authoritative check).
    fn matches_loggers(&self, import_name: &str, loggers: &[String]) -> bool {
        let import_lower = import_name.to_lowercase();

        for logger in loggers {
            let logger_lower = logger.to_lowercase();

            // Exact match
            if logger_lower == import_lower {
                return true;
            }

            // Logger is a sub-path of import (namespace package)
            // e.g., logger "jaraco.abode" validates import "jaraco"
            if logger_lower.starts_with(&format!("{}.", import_lower)) {
                return true;
            }

            // Import is a sub-path of logger
            // e.g., import "aioesphomeapi" with logger "aioesphomeapi"
            if import_lower.starts_with(&format!("{}.", logger_lower)) {
                return true;
            }
        }

        false
    }
}

impl ManifestProvider for HomeAssistantManifest {
    fn is_valid_import(&self, import_name: &str, file_path: &Path) -> bool {
        // Skip template files entirely
        if self.is_template_file(file_path) {
            return true;
        }

        // Skip Home Assistant internal packages (not on PyPI)
        if is_homeassistant_internal(import_name) {
            return true;
        }

        // Check global requirements first
        if self.fuzzy_match_requirements(import_name, &self.global_requirements) {
            return true;
        }

        // Check known submodules against global requirements
        if is_known_submodule(import_name, &self.global_requirements) {
            return true;
        }

        // Find which component this file belongs to
        let component_dir = match self.find_component(file_path) {
            Some(dir) => dir,
            None => return false,
        };

        let component = match self.component_manifests.get(component_dir) {
            Some(c) => c,
            None => return false,
        };

        // Check loggers field first (authoritative)
        if !component.loggers.is_empty() && self.matches_loggers(import_name, &component.loggers) {
            return true;
        }

        // Check known submodules against component requirements
        if is_known_submodule(import_name, &component.requirements) {
            return true;
        }

        // Fallback to fuzzy matching on requirements
        self.fuzzy_match_requirements(import_name, &component.requirements)
    }

    fn get_declared_imports(&self, file_path: &Path) -> Vec<String> {
        let mut imports = Vec::new();

        // Add global requirements
        for req in &self.global_requirements {
            imports.push(extract_package_name(req));
        }

        // Add component-specific imports
        if let Some(component_dir) = self.find_component(file_path) {
            if let Some(component) = self.component_manifests.get(component_dir) {
                for req in &component.requirements {
                    imports.push(extract_package_name(req));
                }
                imports.extend(component.loggers.clone());
            }
        }

        imports
    }

    fn get_scope(&self, file_path: &Path) -> Option<String> {
        self.find_component(file_path)
            .and_then(|dir| {
                self.component_manifests
                    .get(dir)
                    .map(|c| c.domain.clone())
            })
            .or_else(|| {
                // Fallback to directory name
                self.find_component(file_path)
                    .and_then(|dir| dir.file_name())
                    .and_then(|name| name.to_str())
                    .map(|s| s.to_string())
            })
    }

    fn stats(&self) -> ManifestStats {
        let package_count: usize = self.component_manifests
            .values()
            .map(|c| c.requirements.len() + c.loggers.len())
            .sum::<usize>()
            + self.global_requirements.len();

        ManifestStats {
            scoped_count: self.component_manifests.len(),
            package_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_ha_structure(temp: &TempDir) -> PathBuf {
        let root = temp.path();
        let components = root.join("homeassistant/components");
        fs::create_dir_all(&components).unwrap();
        root.to_path_buf()
    }

    fn create_component(root: &Path, name: &str, manifest: &str) {
        let component_dir = root.join("homeassistant/components").join(name);
        fs::create_dir_all(&component_dir).unwrap();
        fs::write(component_dir.join("manifest.json"), manifest).unwrap();
        fs::write(component_dir.join("__init__.py"), "").unwrap();
    }

    #[test]
    fn test_parse_component_manifest() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "switchbot",
            r#"{
                "domain": "switchbot",
                "requirements": ["pyswitchbot==0.40.0"],
                "loggers": ["switchbot"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        assert_eq!(manifest.component_manifests.len(), 1);

        let file_path = root.join("homeassistant/components/switchbot/__init__.py");
        assert!(manifest.is_valid_import("switchbot", &file_path));
    }

    #[test]
    fn test_loggers_field_authoritative() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "abode",
            r#"{
                "domain": "abode",
                "requirements": ["jaraco.abode==5.2.0"],
                "loggers": ["jaraco.abode", "abodepy"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/abode/__init__.py");

        // Logger exact match
        assert!(manifest.is_valid_import("abodepy", &file_path));

        // Namespace package from logger
        assert!(manifest.is_valid_import("jaraco", &file_path));
    }

    #[test]
    fn test_fuzzy_match_tuya() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "tuya",
            r#"{
                "domain": "tuya",
                "requirements": ["tuya-device-sharing-sdk==0.2.1"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/tuya/__init__.py");

        // This is the actual import used in the tuya component
        assert!(manifest.is_valid_import("tuya_sharing", &file_path));
    }

    #[test]
    fn test_fuzzy_match_paho() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "mqtt",
            r#"{
                "domain": "mqtt",
                "requirements": ["paho-mqtt==2.1.0"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/mqtt/__init__.py");

        // Import is just the first part
        assert!(manifest.is_valid_import("paho", &file_path));
    }

    #[test]
    fn test_fuzzy_match_synology() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "synology_dsm",
            r#"{
                "domain": "synology_dsm",
                "requirements": ["py-synologydsm-api==2.5.3"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/synology_dsm/__init__.py");

        // py- prefix and -api suffix stripped
        assert!(manifest.is_valid_import("synology_dsm", &file_path));
        // Also test with synologydsm (no underscore)
        assert!(manifest.is_valid_import("synologydsm", &file_path));
    }

    #[test]
    fn test_template_files_skipped() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();

        let template_file = root.join("script/scaffold/templates/integration/__init__.py");
        // Template files should always return true (skip validation)
        assert!(manifest.is_valid_import("any_import", &template_file));
    }

    #[test]
    fn test_get_scope() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "mqtt",
            r#"{
                "domain": "mqtt",
                "requirements": []
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/mqtt/__init__.py");

        assert_eq!(manifest.get_scope(&file_path), Some("mqtt".to_string()));
    }

    #[test]
    fn test_known_submodule_didl_lite() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "dlna_dmr",
            r#"{
                "domain": "dlna_dmr",
                "requirements": ["async-upnp-client==0.38.0"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/dlna_dmr/__init__.py");

        // didl_lite is a submodule of async-upnp-client
        assert!(manifest.is_valid_import("didl_lite", &file_path));
    }

    #[test]
    fn test_known_submodule_inelsmqtt() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "inels",
            r#"{
                "domain": "inels",
                "requirements": ["elkoep-aio-mqtt==0.1.0b4"]
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/inels/__init__.py");

        // inelsmqtt is a submodule of elkoep-aio-mqtt
        assert!(manifest.is_valid_import("inelsmqtt", &file_path));
    }

    #[test]
    fn test_homeassistant_internal_packages() {
        let temp = TempDir::new().unwrap();
        let root = create_ha_structure(&temp);

        create_component(
            &root,
            "frontend",
            r#"{
                "domain": "frontend",
                "requirements": []
            }"#,
        );

        let manifest = HomeAssistantManifest::from_root(&root).unwrap();
        let file_path = root.join("homeassistant/components/frontend/__init__.py");

        // These are internal packages, not on PyPI
        assert!(manifest.is_valid_import("hass_frontend", &file_path));
        assert!(manifest.is_valid_import("homeassistant_frontend", &file_path));
        assert!(manifest.is_valid_import("insteon_frontend", &file_path));
    }
}

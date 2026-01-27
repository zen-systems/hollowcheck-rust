//! Standard Python manifest provider.
//!
//! Parses standard Python project files:
//! - pyproject.toml
//! - requirements.txt and requirements*.txt
//! - setup.cfg
//! - setup.py (basic parsing)

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::utils::{extract_package_name, import_matches_package};
use super::{ManifestProvider, ManifestStats};

/// Standard Python manifest provider.
///
/// Validates imports against packages declared in standard Python
/// project configuration files.
pub struct PythonManifest {
    /// Project root directory
    root: PathBuf,
    /// All declared packages (normalized names)
    packages: HashSet<String>,
}

impl PythonManifest {
    /// Create a new PythonManifest by scanning the project root.
    pub fn from_root(root: &Path) -> anyhow::Result<Self> {
        let mut manifest = Self {
            root: root.to_path_buf(),
            packages: HashSet::new(),
        };

        // Parse all requirement sources
        manifest.parse_requirements_txt()?;
        manifest.parse_pyproject_toml()?;
        manifest.parse_setup_cfg()?;

        Ok(manifest)
    }

    /// Parse requirements*.txt files at root.
    fn parse_requirements_txt(&mut self) -> anyhow::Result<()> {
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("requirements") && name.ends_with(".txt") {
                        self.parse_requirements_file(&path)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse a single requirements.txt file.
    fn parse_requirements_file(&mut self, path: &Path) -> anyhow::Result<()> {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();

                // Skip comments and empty lines
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                // Skip options like -r, -e, --index-url
                if line.starts_with('-') {
                    continue;
                }

                let pkg_name = extract_package_name(line);
                if !pkg_name.is_empty() {
                    self.packages.insert(pkg_name.to_lowercase());
                }
            }
        }
        Ok(())
    }

    /// Parse pyproject.toml dependencies.
    fn parse_pyproject_toml(&mut self) -> anyhow::Result<()> {
        let path = self.root.join("pyproject.toml");
        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path)?;

        // Simple parsing for [project.dependencies] or [tool.poetry.dependencies]
        let mut in_deps = false;
        let mut brace_depth = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Track section headers
            if trimmed.starts_with("[project.dependencies]")
                || trimmed.starts_with("[tool.poetry.dependencies]")
                || trimmed.contains("dependencies = [")
            {
                in_deps = true;
                if trimmed.contains('[') && !trimmed.contains(']') {
                    brace_depth = 1;
                }
                continue;
            }

            // Exit section on new section header
            if trimmed.starts_with('[') && !trimmed.contains("dependencies") {
                in_deps = false;
                continue;
            }

            if in_deps {
                // Track array depth
                brace_depth += trimmed.matches('[').count();
                brace_depth = brace_depth.saturating_sub(trimmed.matches(']').count());

                if brace_depth == 0 && trimmed.starts_with(']') {
                    in_deps = false;
                    continue;
                }

                // Extract package from various formats:
                // "requests>=2.0"
                // requests = "^2.0"
                // requests = {version = "^2.0"}

                // First, strip quotes and commas
                let cleaned = trimmed.trim_matches(|c| c == '"' || c == ',' || c == '\'');

                // Use extract_package_name which handles all version specifiers
                let pkg_name = extract_package_name(cleaned);

                if !pkg_name.is_empty() && !pkg_name.starts_with('#') && !pkg_name.starts_with('[') {
                    self.packages.insert(pkg_name.to_lowercase());
                }
            }
        }

        Ok(())
    }

    /// Parse setup.cfg [options] install_requires.
    fn parse_setup_cfg(&mut self) -> anyhow::Result<()> {
        let path = self.root.join("setup.cfg");
        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path)?;
        let mut in_install_requires = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('[') {
                in_install_requires = false;
            }

            if trimmed == "install_requires =" || trimmed.starts_with("install_requires=") {
                in_install_requires = true;
                // Check for inline value
                if let Some(eq_pos) = trimmed.find('=') {
                    let value = trimmed[eq_pos + 1..].trim();
                    if !value.is_empty() {
                        let pkg = extract_package_name(value);
                        if !pkg.is_empty() {
                            self.packages.insert(pkg.to_lowercase());
                        }
                    }
                }
                continue;
            }

            if in_install_requires {
                // Continuation lines are indented
                if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                    in_install_requires = false;
                    continue;
                }

                let pkg = extract_package_name(trimmed);
                if !pkg.is_empty() {
                    self.packages.insert(pkg.to_lowercase());
                }
            }
        }

        Ok(())
    }
}

impl ManifestProvider for PythonManifest {
    fn is_valid_import(&self, import_name: &str, _file_path: &Path) -> bool {
        let import_lower = import_name.to_lowercase();

        // Direct match
        if self.packages.contains(&import_lower) {
            return true;
        }

        // Check using common patterns
        for pkg in &self.packages {
            if import_matches_package(&import_lower, pkg) {
                return true;
            }
        }

        false
    }

    fn get_declared_imports(&self, _file_path: &Path) -> Vec<String> {
        self.packages.iter().cloned().collect()
    }

    fn get_scope(&self, _file_path: &Path) -> Option<String> {
        // Standard Python projects don't have component scoping
        self.root
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }

    fn stats(&self) -> ManifestStats {
        ManifestStats {
            scoped_count: 0,
            package_count: self.packages.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_requirements_txt() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("requirements.txt"),
            r#"
# Comment
requests>=2.0
flask==2.0.1
numpy~=1.20
pandas[sql]>=1.3.0
-r other-requirements.txt
--index-url https://pypi.org/simple
"#,
        )
        .unwrap();

        let manifest = PythonManifest::from_root(temp.path()).unwrap();

        assert!(manifest.packages.contains("requests"));
        assert!(manifest.packages.contains("flask"));
        assert!(manifest.packages.contains("numpy"));
        assert!(manifest.packages.contains("pandas"));
        assert!(!manifest.packages.contains("-r"));
    }

    #[test]
    fn test_parse_pyproject_toml() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
name = "myproject"
dependencies = [
    "requests>=2.0",
    "flask",
]
"#,
        )
        .unwrap();

        let manifest = PythonManifest::from_root(temp.path()).unwrap();

        assert!(manifest.packages.contains("requests"));
        assert!(manifest.packages.contains("flask"));
    }

    #[test]
    fn test_is_valid_import_py_prefix() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("requirements.txt"),
            "pyswitchbot==0.40.0\n",
        )
        .unwrap();

        let manifest = PythonManifest::from_root(temp.path()).unwrap();
        let file = temp.path().join("test.py");

        // Should match via py prefix stripping
        assert!(manifest.is_valid_import("switchbot", &file));
    }

    #[test]
    fn test_is_valid_import_namespace() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("requirements.txt"),
            "jaraco.abode>=1.0\n",
        )
        .unwrap();

        let manifest = PythonManifest::from_root(temp.path()).unwrap();
        let file = temp.path().join("test.py");

        // Should match namespace package
        assert!(manifest.is_valid_import("jaraco", &file));
    }

    #[test]
    fn test_stats() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("requirements.txt"),
            "requests\nflask\nnumpy\n",
        )
        .unwrap();

        let manifest = PythonManifest::from_root(temp.path()).unwrap();
        let stats = manifest.stats();

        assert_eq!(stats.scoped_count, 0);
        assert_eq!(stats.package_count, 3);
    }
}

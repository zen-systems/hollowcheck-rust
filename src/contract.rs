//! Contract schema definitions for hollowcheck.
//!
//! A contract defines the quality requirements for a codebase.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Top-level contract definition.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Contract {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// "code" (default) or "prose"
    #[serde(default)]
    pub mode: Option<String>,
    /// Whether to include test files in analysis (default: false)
    #[serde(default)]
    pub include_test_files: Option<bool>,
    /// Glob patterns for paths to exclude from analysis (e.g., "**/docs/**", "**/examples/**")
    #[serde(default)]
    pub excluded_paths: Vec<String>,
    #[serde(default)]
    pub required_files: Vec<RequiredFile>,
    #[serde(default)]
    pub required_symbols: Vec<RequiredSymbol>,
    #[serde(default)]
    pub forbidden_patterns: Vec<ForbiddenPattern>,
    #[serde(default)]
    pub mock_signatures: Option<MockSignaturesConfig>,
    #[serde(default)]
    pub complexity: Vec<ComplexityRequirement>,
    #[serde(default)]
    pub required_tests: Vec<RequiredTest>,
    #[serde(default)]
    pub coverage_threshold: Option<f64>,
    #[serde(default)]
    pub prose: Option<ProseConfig>,
    #[serde(default)]
    pub dependency_verification: Option<DependencyVerificationConfig>,
    #[serde(default)]
    pub god_objects: Option<GodObjectContractConfig>,
    /// Whether to detect hollow TODOs (TODOs without meaningful context). Default: true
    #[serde(default)]
    pub hollow_todos: Option<HollowTodosConfig>,
}

impl Contract {
    /// Parse a contract from a YAML file.
    pub fn parse_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path.as_ref())?;
        let contract: Contract = serde_yaml::from_str(&content)?;
        Ok(contract)
    }

    /// Returns whether to include test files (defaults to false).
    pub fn should_include_test_files(&self) -> bool {
        self.include_test_files.unwrap_or(false)
    }

    /// Returns the analysis mode (defaults to "code").
    pub fn get_mode(&self) -> &str {
        self.mode.as_deref().unwrap_or("code")
    }

    /// Check if a path should be excluded based on excluded_paths patterns.
    /// Uses globset for matching, which supports `**` for recursive directory matching.
    pub fn is_path_excluded(&self, path: &Path) -> bool {
        if self.excluded_paths.is_empty() {
            return false;
        }

        let path_str = path.to_string_lossy();

        for pattern in &self.excluded_paths {
            if let Ok(glob) = globset::Glob::new(pattern) {
                let matcher = glob.compile_matcher();
                if matcher.is_match(&*path_str) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns whether hollow TODO detection is enabled (defaults to true).
    pub fn detect_hollow_todos(&self) -> bool {
        self.hollow_todos
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(true)
    }
}

/// A file that must exist.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequiredFile {
    pub path: String,
    #[serde(default)]
    pub required: bool,
}

/// Kind of symbol (function, method, type, const).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Type,
    Const,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "function"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Const => write!(f, "const"),
        }
    }
}

/// A symbol that must be defined.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequiredSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
}

/// A regex pattern that must not appear in the code.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForbiddenPattern {
    pub pattern: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A regex pattern identifying mock/placeholder data.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockSignature {
    pub pattern: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Configuration for mock signature detection.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MockSignaturesConfig {
    #[serde(default)]
    pub patterns: Vec<MockSignature>,
    /// Whether to skip test files (default: true)
    #[serde(default)]
    pub skip_test_files: Option<bool>,
    /// Severity for test files: "info", "warning", or "" (skip)
    #[serde(default)]
    pub test_file_severity: Option<String>,
}

impl MockSignaturesConfig {
    /// Returns whether to skip test files (defaults to true).
    pub fn should_skip_test_files(&self) -> bool {
        self.skip_test_files.unwrap_or(true)
    }

    /// Returns the severity to use for test files.
    /// Returns None if test files should be skipped entirely.
    pub fn get_test_file_severity(&self) -> Option<&str> {
        if self.should_skip_test_files() {
            return None;
        }
        match &self.test_file_severity {
            Some(s) if !s.is_empty() => Some(s.as_str()),
            _ => Some("warning"),
        }
    }
}

/// Minimum cyclomatic complexity requirement for a symbol.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComplexityRequirement {
    pub symbol: String,
    #[serde(default)]
    pub file: Option<String>,
    pub min_complexity: i32,
}

/// A test function that must exist.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequiredTest {
    pub name: String,
    #[serde(default)]
    pub file: Option<String>,
}

/// Configuration for prose analysis.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProseConfig {
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filler_threshold: Option<i32>,
    #[serde(default)]
    pub weasel_threshold: Option<i32>,
    #[serde(default)]
    pub weights: Option<ProseWeightsConfig>,
    #[serde(default)]
    pub density: Option<ProseDensityConfig>,
    #[serde(default)]
    pub custom_fillers: Vec<ProsePattern>,
    #[serde(default)]
    pub custom_weasels: Vec<ProsePattern>,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

/// Weights for different prose issues.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProseWeightsConfig {
    #[serde(default)]
    pub filler: Option<f64>,
    #[serde(default)]
    pub weasel: Option<f64>,
    #[serde(default)]
    pub density: Option<f64>,
    #[serde(default)]
    pub structure: Option<f64>,
}

/// Configuration for density analysis.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProseDensityConfig {
    #[serde(default)]
    pub min_section_words: Option<i32>,
    #[serde(default)]
    pub low_threshold: Option<f64>,
    #[serde(default)]
    pub high_threshold: Option<f64>,
}

/// A custom pattern for prose analysis.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProsePattern {
    pub pattern: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub weight: Option<f64>,
}

/// Configuration for dependency verification (hallucinated dependency detection).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DependencyVerificationConfig {
    /// Whether dependency verification is enabled (default: true when present)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Registry-specific configuration
    #[serde(default)]
    pub registries: RegistriesConfig,
    /// Package names or glob patterns to skip verification for (e.g., internal packages)
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// How long to cache registry responses in hours (default: 24)
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_hours: u32,
    /// If true, treat registry timeouts as errors; if false, warn but pass (default: false)
    #[serde(default)]
    pub fail_on_timeout: bool,
}

fn default_true() -> bool {
    true
}

fn default_cache_ttl() -> u32 {
    24
}

impl DependencyVerificationConfig {
    /// Returns whether dependency verification is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Configuration for individual registries.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistriesConfig {
    #[serde(default = "RegistryConfig::default_enabled")]
    pub pypi: RegistryConfig,
    #[serde(default = "RegistryConfig::default_enabled")]
    pub npm: RegistryConfig,
    #[serde(default = "RegistryConfig::default_enabled")]
    pub crates: RegistryConfig,
    #[serde(default = "RegistryConfig::default_enabled")]
    pub go: RegistryConfig,
}

impl Default for RegistriesConfig {
    fn default() -> Self {
        Self {
            pypi: RegistryConfig::default_enabled(),
            npm: RegistryConfig::default_enabled(),
            crates: RegistryConfig::default_enabled(),
            go: RegistryConfig::default_enabled(),
        }
    }
}

/// Configuration for a single registry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryConfig {
    /// Whether this registry check is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Request timeout in milliseconds (default: 5000)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

impl RegistryConfig {
    pub fn default_enabled() -> Self {
        Self {
            enabled: true,
            timeout_ms: 5000,
        }
    }
}

/// Configuration for god object detection in the contract.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GodObjectContractConfig {
    /// Whether god object detection is enabled (default: true when present)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum lines per file before flagging (default: 500)
    #[serde(default)]
    pub max_file_lines: Option<usize>,
    /// Maximum lines per function before flagging (default: 50)
    #[serde(default)]
    pub max_function_lines: Option<usize>,
    /// Maximum cyclomatic complexity per function (default: 15)
    #[serde(default)]
    pub max_function_complexity: Option<usize>,
    /// Maximum functions per file (default: 20)
    #[serde(default)]
    pub max_functions_per_file: Option<usize>,
    /// Maximum methods per class (default: 15)
    #[serde(default)]
    pub max_class_methods: Option<usize>,
}

impl GodObjectContractConfig {
    /// Returns whether god object detection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Configuration for hollow TODO detection.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HollowTodosConfig {
    /// Whether hollow TODO detection is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Validate a contract for correctness.
pub fn validate(contract: &Contract) -> anyhow::Result<()> {
    // Validate mode
    if let Some(mode) = &contract.mode {
        if mode != "code" && mode != "prose" {
            anyhow::bail!("invalid mode {:?}, must be 'code' or 'prose'", mode);
        }
    }

    // Validate forbidden patterns compile
    for p in &contract.forbidden_patterns {
        regex::Regex::new(&p.pattern)
            .map_err(|e| anyhow::anyhow!("invalid forbidden pattern {:?}: {}", p.pattern, e))?;
    }

    // Validate mock signature patterns compile
    if let Some(mock_cfg) = &contract.mock_signatures {
        for s in &mock_cfg.patterns {
            regex::Regex::new(&s.pattern)
                .map_err(|e| anyhow::anyhow!("invalid mock signature {:?}: {}", s.pattern, e))?;
        }
    }

    // Validate excluded_paths glob patterns compile
    for pattern in &contract.excluded_paths {
        globset::Glob::new(pattern)
            .map_err(|e| anyhow::anyhow!("invalid excluded_paths pattern {:?}: {}", pattern, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_contract() {
        let yaml = r#"
version: "1.0"
name: "Test Contract"
required_files:
  - path: "go.mod"
    required: true
forbidden_patterns:
  - pattern: "TODO"
    description: "Remove TODO comments"
"#;
        let contract: Contract = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(contract.name, "Test Contract");
        assert_eq!(contract.required_files.len(), 1);
        assert_eq!(contract.forbidden_patterns.len(), 1);
    }

    #[test]
    fn test_mock_signatures_defaults() {
        let cfg = MockSignaturesConfig::default();
        assert!(cfg.should_skip_test_files());
        assert!(cfg.get_test_file_severity().is_none());
    }
}

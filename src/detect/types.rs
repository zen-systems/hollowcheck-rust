//! Core types for detection results.

use serde::{Deserialize, Serialize};

/// Severity levels for violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" => Ok(Severity::Warning),
            "info" => Ok(Severity::Info),
            _ => Err(format!("unknown severity: {}", s)),
        }
    }
}

/// Rule names for different violation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViolationRule {
    #[serde(rename = "forbidden_pattern")]
    ForbiddenPattern,
    #[serde(rename = "mock_data")]
    MockData,
    #[serde(rename = "missing_file")]
    MissingFile,
    #[serde(rename = "missing_symbol")]
    MissingSymbol,
    #[serde(rename = "low_complexity")]
    LowComplexity,
    #[serde(rename = "missing_test")]
    MissingTest,
    #[serde(rename = "hallucinated_dependency")]
    HallucinatedDependency,
    // God object rules
    #[serde(rename = "god_file")]
    GodFile,
    #[serde(rename = "god_function")]
    GodFunction,
    #[serde(rename = "god_class")]
    GodClass,
    // Prose rules
    #[serde(rename = "filler_phrase")]
    FillerPhrase,
    #[serde(rename = "weasel_word")]
    WeaselWord,
    #[serde(rename = "low_density")]
    LowDensity,
    #[serde(rename = "prose_repetitive_opener")]
    ProseRepetitiveOpener,
    #[serde(rename = "prose_middle_sag")]
    ProseMiddleSag,
    #[serde(rename = "prose_weak_transition")]
    ProseWeakTransition,
}

impl ViolationRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            ViolationRule::ForbiddenPattern => "forbidden_pattern",
            ViolationRule::MockData => "mock_data",
            ViolationRule::MissingFile => "missing_file",
            ViolationRule::MissingSymbol => "missing_symbol",
            ViolationRule::LowComplexity => "low_complexity",
            ViolationRule::MissingTest => "missing_test",
            ViolationRule::HallucinatedDependency => "hallucinated_dependency",
            ViolationRule::GodFile => "god_file",
            ViolationRule::GodFunction => "god_function",
            ViolationRule::GodClass => "god_class",
            ViolationRule::FillerPhrase => "filler_phrase",
            ViolationRule::WeaselWord => "weasel_word",
            ViolationRule::LowDensity => "low_density",
            ViolationRule::ProseRepetitiveOpener => "prose_repetitive_opener",
            ViolationRule::ProseMiddleSag => "prose_middle_sag",
            ViolationRule::ProseWeakTransition => "prose_weak_transition",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "forbidden_pattern" => Some(ViolationRule::ForbiddenPattern),
            "mock_data" => Some(ViolationRule::MockData),
            "missing_file" => Some(ViolationRule::MissingFile),
            "missing_symbol" => Some(ViolationRule::MissingSymbol),
            "low_complexity" => Some(ViolationRule::LowComplexity),
            "missing_test" => Some(ViolationRule::MissingTest),
            "hallucinated_dependency" => Some(ViolationRule::HallucinatedDependency),
            "god_file" => Some(ViolationRule::GodFile),
            "god_function" => Some(ViolationRule::GodFunction),
            "god_class" => Some(ViolationRule::GodClass),
            "filler_phrase" => Some(ViolationRule::FillerPhrase),
            "weasel_word" => Some(ViolationRule::WeaselWord),
            "low_density" => Some(ViolationRule::LowDensity),
            "prose_repetitive_opener" => Some(ViolationRule::ProseRepetitiveOpener),
            "prose_middle_sag" => Some(ViolationRule::ProseMiddleSag),
            "prose_weak_transition" => Some(ViolationRule::ProseWeakTransition),
            _ => None,
        }
    }
}

impl std::fmt::Display for ViolationRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single detected issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub rule: ViolationRule,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub severity: Severity,
}

impl Violation {
    /// Create a unique key for this violation (for deduplication/comparison).
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.rule, self.file, self.message)
    }
}

/// Results of running detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectionResult {
    pub violations: Vec<Violation>,
    /// Violations that were suppressed by inline comments
    #[serde(default)]
    pub suppressed: Vec<super::SuppressedViolation>,
    /// Violations not present in baseline (baseline mode only)
    #[serde(default)]
    pub new_violations: Vec<Violation>,
    /// Number of files scanned
    pub scanned: usize,
    /// Git ref used for baseline (if baseline mode)
    #[serde(default)]
    pub baseline_ref: Option<String>,
}

impl DetectionResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another result into this one.
    pub fn merge(&mut self, other: DetectionResult) {
        self.violations.extend(other.violations);
        self.suppressed.extend(other.suppressed);
        self.scanned += other.scanned;
    }

    /// Add a violation to the result.
    pub fn add_violation(&mut self, violation: Violation) {
        self.violations.push(violation);
    }

    /// Number of suppressed violations.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed.len()
    }

    /// Check if there are any error-severity violations.
    pub fn has_errors(&self) -> bool {
        self.violations
            .iter()
            .any(|v| v.severity == Severity::Error)
    }

    /// Number of new violations (baseline mode).
    pub fn new_violation_count(&self) -> usize {
        self.new_violations.len()
    }

    /// Check if this result was generated in baseline mode.
    pub fn is_baseline_mode(&self) -> bool {
        self.baseline_ref.is_some()
    }
}

/// Check if two violations match (ignoring line numbers).
/// Line numbers are ignored because code changes can shift them.
#[allow(dead_code)]
pub fn violations_match(a: &Violation, b: &Violation) -> bool {
    a.rule == b.rule && a.file == b.file && a.message == b.message
}

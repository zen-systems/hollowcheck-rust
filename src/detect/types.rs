//! Core types for detection results.

use serde::{Deserialize, Serialize};

/// Severity levels for violations.
/// Critical and Error are "hard" violations that count toward the hollowness score.
/// Warning and Info are "soft" violations reported for awareness but don't fail the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Missing implementation, hallucinated deps - absolute blockers
    Critical,
    /// Forbidden patterns, low complexity - serious issues
    Error,
    /// God files, context-less TODOs - code smells
    Warning,
    /// Informational issues like example.com in docs
    Info,
}

impl Severity {
    /// Returns true if this severity should count toward the hollowness score.
    /// Only Critical and Error severities affect the score.
    pub fn counts_toward_score(&self) -> bool {
        matches!(self, Severity::Critical | Severity::Error)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
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
            "critical" => Ok(Severity::Critical),
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
    /// Hollow TODO - a TODO without meaningful context
    #[serde(rename = "hollow_todo")]
    HollowTodo,
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
            ViolationRule::HollowTodo => "hollow_todo",
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
            "hollow_todo" => Some(ViolationRule::HollowTodo),
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

    /// Returns the default severity for this rule type.
    /// Critical: Missing implementations, hallucinated dependencies
    /// Error: Forbidden patterns, low complexity
    /// Warning: God objects, mock data, hollow TODOs
    /// Info: Weak prose issues
    pub fn default_severity(&self) -> Severity {
        match self {
            // Critical - absolute blockers
            ViolationRule::MissingFile => Severity::Critical,
            ViolationRule::MissingSymbol => Severity::Critical,
            ViolationRule::HallucinatedDependency => Severity::Critical,

            // Error - serious issues
            ViolationRule::ForbiddenPattern => Severity::Error,
            ViolationRule::LowComplexity => Severity::Error,

            // Warning - code smells / informational
            ViolationRule::GodFile => Severity::Warning,
            ViolationRule::GodFunction => Severity::Warning,
            ViolationRule::GodClass => Severity::Warning,
            ViolationRule::MockData => Severity::Warning,
            ViolationRule::MissingTest => Severity::Warning,
            ViolationRule::HollowTodo => Severity::Warning,

            // Prose rules - mostly warnings/info
            ViolationRule::FillerPhrase => Severity::Warning,
            ViolationRule::WeaselWord => Severity::Warning,
            ViolationRule::LowDensity => Severity::Warning,
            ViolationRule::ProseRepetitiveOpener => Severity::Warning,
            ViolationRule::ProseMiddleSag => Severity::Error,
            ViolationRule::ProseWeakTransition => Severity::Info,
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

    /// Check if there are any critical or error severity violations.
    pub fn has_errors(&self) -> bool {
        self.violations
            .iter()
            .any(|v| matches!(v.severity, Severity::Critical | Severity::Error))
    }

    /// Check if there are any critical severity violations.
    pub fn has_critical(&self) -> bool {
        self.violations
            .iter()
            .any(|v| v.severity == Severity::Critical)
    }

    /// Count violations that count toward the hollowness score (Critical + Error only).
    pub fn scoring_violation_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|v| v.severity.counts_toward_score())
            .count()
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

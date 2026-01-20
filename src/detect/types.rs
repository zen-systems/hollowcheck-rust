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
    /// Error: Low complexity (stub implementations)
    /// Warning: Forbidden patterns (TODOs), god objects, mock data, hollow TODOs
    /// Info: Weak prose issues
    pub fn default_severity(&self) -> Severity {
        match self {
            // Critical - absolute blockers
            ViolationRule::MissingFile => Severity::Critical,
            ViolationRule::MissingSymbol => Severity::Critical,
            ViolationRule::HallucinatedDependency => Severity::Critical,

            // Error - serious issues that should block CI
            ViolationRule::LowComplexity => Severity::Error,

            // Warning - code smells that don't affect scoring
            ViolationRule::ForbiddenPattern => Severity::Warning,
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
    /// Includes rule, file, line, and message to ensure exact duplicates are caught.
    pub fn key(&self) -> String {
        format!("{}|{}|{}|{}", self.rule, self.file, self.line, self.message)
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

    /// Deduplicate violations by removing exact duplicates (same file, line, rule, message).
    /// This prevents the same violation from being reported multiple times.
    pub fn deduplicate(&mut self) {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        self.violations.retain(|v| {
            let key = v.key();
            if seen.contains(&key) {
                false
            } else {
                seen.insert(key);
                true
            }
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_violation(rule: ViolationRule, file: &str, line: usize, message: &str) -> Violation {
        Violation {
            rule,
            message: message.to_string(),
            file: file.to_string(),
            line,
            severity: rule.default_severity(),
        }
    }

    #[test]
    fn test_violation_key() {
        let v = make_violation(ViolationRule::GodFunction, "src/main.rs", 10, "complexity exceeded");
        assert_eq!(v.key(), "god_function|src/main.rs|10|complexity exceeded");
    }

    #[test]
    fn test_deduplicate_removes_exact_duplicates() {
        let mut result = DetectionResult::new();

        // Add the same violation twice
        let v1 = make_violation(ViolationRule::GodFunction, "src/main.rs", 10, "function too complex");
        let v2 = make_violation(ViolationRule::GodFunction, "src/main.rs", 10, "function too complex");
        result.add_violation(v1);
        result.add_violation(v2);

        assert_eq!(result.violations.len(), 2);
        result.deduplicate();
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn test_deduplicate_keeps_different_violations() {
        let mut result = DetectionResult::new();

        // Add violations with different messages (same file/line but different reason)
        let v1 = make_violation(ViolationRule::GodFunction, "src/main.rs", 10, "complexity 18 exceeds 15");
        let v2 = make_violation(ViolationRule::GodFunction, "src/main.rs", 10, "~73 lines exceeds 50");
        result.add_violation(v1);
        result.add_violation(v2);

        assert_eq!(result.violations.len(), 2);
        result.deduplicate();
        assert_eq!(result.violations.len(), 2); // Both should remain (different messages)
    }

    #[test]
    fn test_deduplicate_keeps_violations_on_different_lines() {
        let mut result = DetectionResult::new();

        // Add same message but different lines
        let v1 = make_violation(ViolationRule::ForbiddenPattern, "src/main.rs", 10, "TODO found");
        let v2 = make_violation(ViolationRule::ForbiddenPattern, "src/main.rs", 20, "TODO found");
        result.add_violation(v1);
        result.add_violation(v2);

        assert_eq!(result.violations.len(), 2);
        result.deduplicate();
        assert_eq!(result.violations.len(), 2); // Both should remain (different lines)
    }

    #[test]
    fn test_deduplicate_keeps_violations_in_different_files() {
        let mut result = DetectionResult::new();

        // Add same violation in different files
        let v1 = make_violation(ViolationRule::GodFile, "src/a.rs", 1, "file too large");
        let v2 = make_violation(ViolationRule::GodFile, "src/b.rs", 1, "file too large");
        result.add_violation(v1);
        result.add_violation(v2);

        assert_eq!(result.violations.len(), 2);
        result.deduplicate();
        assert_eq!(result.violations.len(), 2); // Both should remain (different files)
    }

    #[test]
    fn test_forbidden_pattern_severity_is_warning() {
        assert_eq!(ViolationRule::ForbiddenPattern.default_severity(), Severity::Warning);
    }

    #[test]
    fn test_hallucinated_dependency_severity_is_critical() {
        assert_eq!(ViolationRule::HallucinatedDependency.default_severity(), Severity::Critical);
    }

    #[test]
    fn test_god_object_severities_are_warning() {
        assert_eq!(ViolationRule::GodFile.default_severity(), Severity::Warning);
        assert_eq!(ViolationRule::GodFunction.default_severity(), Severity::Warning);
        assert_eq!(ViolationRule::GodClass.default_severity(), Severity::Warning);
    }

    #[test]
    fn test_scoring_only_counts_critical_and_error() {
        let mut result = DetectionResult::new();

        // Add violations of different severities
        result.add_violation(make_violation(ViolationRule::MissingFile, "a.rs", 1, "missing")); // Critical
        result.add_violation(make_violation(ViolationRule::LowComplexity, "b.rs", 1, "stub")); // Error
        result.add_violation(make_violation(ViolationRule::GodFunction, "c.rs", 1, "too complex")); // Warning
        result.add_violation(make_violation(ViolationRule::MockData, "d.rs", 1, "mock found")); // Warning

        // Only Critical and Error should count
        assert_eq!(result.scoring_violation_count(), 2);
    }
}

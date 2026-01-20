//! Scoring and grading system for hollowcheck.
//!
//! Calculates a hollowness score (0-100) based on violation counts and weights.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::contract::Contract;
use crate::detect::{DetectionResult, ViolationRule};

/// Point weights for each violation type.
pub mod points {
    pub const MISSING_FILE: i32 = 20; // critical
    pub const MISSING_SYMBOL: i32 = 15; // critical
    pub const HALLUCINATED_DEPENDENCY: i32 = 15; // critical - same as missing symbol
    pub const FORBIDDEN_PATTERN: i32 = 10; // error
    pub const LOW_COMPLEXITY: i32 = 10; // error
    pub const GOD_FILE: i32 = 8; // warning - architectural smell
    pub const GOD_FUNCTION: i32 = 8; // warning - architectural smell
    pub const GOD_CLASS: i32 = 8; // warning - architectural smell
    pub const MISSING_TEST: i32 = 5; // warning
    pub const MOCK_DATA: i32 = 3; // warning
    pub const HOLLOW_TODO: i32 = 5; // warning - context-less TODO

    // Prose-specific point weights
    pub const FILLER_PHRASE: i32 = 2; // warning
    pub const WEASEL_WORD: i32 = 3; // warning
    pub const LOW_DENSITY: i32 = 5; // warning
    pub const REPETITIVE_STRUCTURE: i32 = 3; // warning
    pub const MIDDLE_SAG: i32 = 8; // error
    pub const WEAK_TRANSITION: i32 = 2; // info
    pub const PROSE_DEFAULT: i32 = 2; // default for prose issues
}

/// Default threshold when the contract doesn't specify one.
pub const DEFAULT_THRESHOLD: i32 = 25;

/// Grade thresholds.
pub mod grades {
    pub const A_MAX: i32 = 10;
    pub const B_MAX: i32 = 25;
    pub const C_MAX: i32 = 50;
    pub const D_MAX: i32 = 75;
}

/// The calculated hollowness score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HollownessScore {
    /// Score from 0-100, higher = more hollow
    pub score: i32,
    /// Letter grade: "A" (0-10), "B" (11-25), "C" (26-50), "D" (51-75), "F" (76-100)
    pub grade: String,
    /// Points by violation category
    pub breakdown: HashMap<String, i32>,
    /// Whether the check passed (score <= threshold)
    pub passed: bool,
    /// The threshold used
    pub threshold: i32,
}

impl HollownessScore {
    /// Get the total points before capping at 100.
    pub fn total_points(&self) -> i32 {
        self.breakdown.values().sum()
    }

    /// Get the number of violations for a given rule.
    pub fn violation_count(&self, rule: &str) -> i32 {
        let points = self.breakdown.get(rule).copied().unwrap_or(0);
        let per_violation = get_points_for_rule(rule);
        if per_violation == 0 {
            return 0;
        }
        points / per_violation
    }
}

/// Get the point weight for a violation rule.
fn get_points_for_rule(rule: &str) -> i32 {
    match rule {
        "missing_file" => points::MISSING_FILE,
        "missing_symbol" => points::MISSING_SYMBOL,
        "hallucinated_dependency" => points::HALLUCINATED_DEPENDENCY,
        "forbidden_pattern" => points::FORBIDDEN_PATTERN,
        "low_complexity" => points::LOW_COMPLEXITY,
        "god_file" => points::GOD_FILE,
        "god_function" => points::GOD_FUNCTION,
        "god_class" => points::GOD_CLASS,
        "missing_test" => points::MISSING_TEST,
        "mock_data" => points::MOCK_DATA,
        "hollow_todo" => points::HOLLOW_TODO,
        // Prose rules
        "filler_phrase" => points::FILLER_PHRASE,
        "weasel_word" => points::WEASEL_WORD,
        "low_density" => points::LOW_DENSITY,
        "prose_repetitive_opener" => points::REPETITIVE_STRUCTURE,
        "prose_middle_sag" => points::MIDDLE_SAG,
        "prose_weak_transition" => points::WEAK_TRANSITION,
        _ => {
            // For any unknown prose rules, return a default
            if rule.starts_with("prose_") {
                points::PROSE_DEFAULT
            } else {
                0
            }
        }
    }
}

/// Get the point weight for a ViolationRule enum.
fn get_points(rule: ViolationRule) -> i32 {
    get_points_for_rule(rule.as_str())
}

/// Determine the letter grade from a score.
fn calculate_grade(score: i32) -> String {
    match score {
        s if s <= grades::A_MAX => "A".to_string(),
        s if s <= grades::B_MAX => "B".to_string(),
        s if s <= grades::C_MAX => "C".to_string(),
        s if s <= grades::D_MAX => "D".to_string(),
        _ => "F".to_string(),
    }
}

/// Calculate the hollowness score from detection results.
/// Only Critical and Error severity violations count toward the score.
/// Warning and Info violations are tracked in breakdown but don't affect pass/fail.
pub fn calculate(result: &DetectionResult, contract: &Contract) -> HollownessScore {
    let mut breakdown: HashMap<String, i32> = HashMap::new();
    let mut scoring_points = 0;

    // Count violations by rule and calculate points
    // Only Critical/Error count toward the score
    for v in &result.violations {
        let points = get_points(v.rule);
        *breakdown.entry(v.rule.as_str().to_string()).or_insert(0) += points;

        // Only add to scoring total if this severity counts toward score
        if v.severity.counts_toward_score() {
            scoring_points += points;
        }
    }

    // Cap at 100
    let score = scoring_points.min(100);

    // Determine threshold (could be extended to read from contract)
    let threshold = DEFAULT_THRESHOLD;
    let _ = contract; // Silence unused warning for now

    HollownessScore {
        score,
        grade: calculate_grade(score),
        breakdown,
        passed: score <= threshold,
        threshold,
    }
}

/// Calculate the hollowness score with a custom threshold.
/// Only Critical and Error severity violations count toward the score.
pub fn calculate_with_threshold(result: &DetectionResult, threshold: i32) -> HollownessScore {
    let mut breakdown: HashMap<String, i32> = HashMap::new();
    let mut scoring_points = 0;

    for v in &result.violations {
        let points = get_points(v.rule);
        *breakdown.entry(v.rule.as_str().to_string()).or_insert(0) += points;

        // Only add to scoring total if this severity counts toward score
        if v.severity.counts_toward_score() {
            scoring_points += points;
        }
    }

    let score = scoring_points.min(100);

    HollownessScore {
        score,
        grade: calculate_grade(score),
        breakdown,
        passed: score <= threshold,
        threshold,
    }
}

/// Calculate a score based only on new violations (baseline mode).
/// The threshold defaults to 0 if not specified (any new violation fails).
/// Only Critical and Error severity violations count toward the score.
pub fn calculate_for_new_violations(result: &DetectionResult, threshold: i32) -> HollownessScore {
    let mut breakdown: HashMap<String, i32> = HashMap::new();
    let mut scoring_points = 0;

    // Only count new violations
    for v in &result.new_violations {
        let points = get_points(v.rule);
        *breakdown.entry(v.rule.as_str().to_string()).or_insert(0) += points;

        // Only add to scoring total if this severity counts toward score
        if v.severity.counts_toward_score() {
            scoring_points += points;
        }
    }

    let score = scoring_points.min(100);

    // For baseline mode, default threshold is 0 (any new violation fails)
    let threshold = if threshold < 0 { 0 } else { threshold };

    HollownessScore {
        score,
        grade: calculate_grade(score),
        breakdown,
        passed: score <= threshold,
        threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Violation;

    fn make_violation(rule: ViolationRule) -> Violation {
        Violation {
            rule,
            message: "test".to_string(),
            file: "test.go".to_string(),
            line: 1,
            severity: rule.default_severity(),
        }
    }

    #[test]
    fn test_calculate_score() {
        let mut result = DetectionResult::new();
        result.add_violation(make_violation(ViolationRule::LowComplexity)); // 10 pts (Error - counts)
        result.add_violation(make_violation(ViolationRule::MockData)); // 3 pts (Warning - doesn't count)

        let contract = Contract::default();
        let score = calculate(&result, &contract);

        // Only Critical/Error count toward score. MockData is Warning.
        assert_eq!(score.score, 10);
        assert_eq!(score.grade, "A");
        assert!(score.passed); // 10 <= 25
        // But breakdown still includes all violations
        assert_eq!(score.breakdown.get("low_complexity"), Some(&10));
        assert_eq!(score.breakdown.get("mock_data"), Some(&3));
    }

    #[test]
    fn test_calculate_score_exceeds_threshold() {
        let mut result = DetectionResult::new();
        // Add violations to exceed threshold
        for _ in 0..3 {
            result.add_violation(make_violation(ViolationRule::MissingFile)); // 20 points each
        }

        let contract = Contract::default();
        let score = calculate(&result, &contract);

        assert_eq!(score.score, 60);
        assert_eq!(score.grade, "D");
        assert!(!score.passed); // 60 > 25
    }

    #[test]
    fn test_calculate_score_capped_at_100() {
        let mut result = DetectionResult::new();
        // Add many violations
        for _ in 0..20 {
            result.add_violation(make_violation(ViolationRule::MissingFile)); // 20 points each = 400
        }

        let contract = Contract::default();
        let score = calculate(&result, &contract);

        assert_eq!(score.score, 100);
        assert_eq!(score.grade, "F");
        assert_eq!(score.total_points(), 400); // Breakdown still has actual total
    }

    #[test]
    fn test_grade_thresholds() {
        assert_eq!(calculate_grade(0), "A");
        assert_eq!(calculate_grade(10), "A");
        assert_eq!(calculate_grade(11), "B");
        assert_eq!(calculate_grade(25), "B");
        assert_eq!(calculate_grade(26), "C");
        assert_eq!(calculate_grade(50), "C");
        assert_eq!(calculate_grade(51), "D");
        assert_eq!(calculate_grade(75), "D");
        assert_eq!(calculate_grade(76), "F");
        assert_eq!(calculate_grade(100), "F");
    }

    #[test]
    fn test_calculate_with_custom_threshold() {
        let mut result = DetectionResult::new();
        result.add_violation(make_violation(ViolationRule::LowComplexity)); // 10 points (Error - counts)

        let score = calculate_with_threshold(&result, 5);
        assert!(!score.passed); // 10 > 5

        let score = calculate_with_threshold(&result, 15);
        assert!(score.passed); // 10 <= 15
    }

    #[test]
    fn test_calculate_for_new_violations() {
        let mut result = DetectionResult::new();
        // Regular violations (not counted in new_violations mode)
        result.add_violation(make_violation(ViolationRule::LowComplexity));
        result.add_violation(make_violation(ViolationRule::LowComplexity));
        // New violations (only these should count, and only Critical/Error)
        result
            .new_violations
            .push(make_violation(ViolationRule::LowComplexity)); // 10 pts (Error - counts)
        result
            .new_violations
            .push(make_violation(ViolationRule::MockData)); // 3 pts (Warning - doesn't count)

        let score = calculate_for_new_violations(&result, 0);
        assert_eq!(score.score, 10); // Only LowComplexity (Error) counts
        assert!(!score.passed); // 10 > 0

        let score = calculate_for_new_violations(&result, 15);
        assert!(score.passed); // 10 <= 15
    }
}

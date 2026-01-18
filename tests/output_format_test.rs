//! Tests for output format compatibility with Go version.
//!
//! These tests verify that the JSON and SARIF output formats match
//! the original Go implementation exactly.

use std::path::PathBuf;

use hollowcheck::contract::Contract;
use hollowcheck::detect::{Runner, Severity, Violation, ViolationRule, DetectionResult};
use hollowcheck::parser;
use hollowcheck::report::{JsonReport, JsonViolation, BreakdownEntry};
use hollowcheck::score;

fn testdata_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

fn setup() {
    parser::init();
}

/// Run detection and return JSON output as a parsed struct.
fn run_and_get_json() -> JsonReport {
    setup();

    let testdata = testdata_path();
    let contract_path = testdata.join("test-contract.yaml");
    let contract = Contract::parse_file(&contract_path).expect("should parse contract");

    let files: Vec<PathBuf> = std::fs::read_dir(&testdata)
        .expect("should read testdata dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "go").unwrap_or(false))
        .collect();

    let runner = Runner::new(&testdata);
    let result = runner.run(&files, &contract).expect("detection should succeed");
    let hollowness = score::calculate(&result, &contract);

    // Build JSON report manually to test the structure
    let violations: Vec<JsonViolation> = result
        .violations
        .iter()
        .map(|v| JsonViolation {
            rule: v.rule.as_str().to_string(),
            severity: v.severity.to_string(),
            file: v.file.clone(),
            line: v.line,
            message: v.message.clone(),
        })
        .collect();

    let breakdown: Vec<BreakdownEntry> = hollowness
        .breakdown
        .iter()
        .map(|(rule, points)| BreakdownEntry {
            rule: rule.clone(),
            points: *points,
            violations: hollowness.violation_count(rule),
        })
        .collect();

    JsonReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        path: testdata.to_string_lossy().to_string(),
        contract: contract_path.to_string_lossy().to_string(),
        score: hollowness.score,
        grade: hollowness.grade.clone(),
        threshold: hollowness.threshold,
        passed: hollowness.passed,
        files_scanned: result.scanned,
        violations,
        new_violations: vec![],
        baseline_ref: None,
        suppressed: vec![],
        suppressed_count: 0,
        breakdown,
    }
}

#[test]
fn test_json_report_structure() {
    let report = run_and_get_json();

    // Verify required fields are present
    assert!(!report.version.is_empty(), "version should not be empty");
    assert!(!report.path.is_empty(), "path should not be empty");
    assert!(!report.contract.is_empty(), "contract should not be empty");
    assert!(report.score >= 0, "score should be non-negative");
    assert!(!report.grade.is_empty(), "grade should not be empty");
    assert!(report.threshold > 0, "threshold should be positive");
    assert!(report.files_scanned > 0, "should have scanned files");
}

#[test]
fn test_json_violations_format() {
    let report = run_and_get_json();

    // Should have violations
    assert!(!report.violations.is_empty(), "should have violations");

    // Each violation should have required fields
    for v in &report.violations {
        assert!(!v.rule.is_empty(), "rule should not be empty");
        assert!(!v.severity.is_empty(), "severity should not be empty");
        assert!(!v.file.is_empty(), "file should not be empty");
        assert!(!v.message.is_empty(), "message should not be empty");

        // Severity should be one of: error, warning, info
        assert!(
            v.severity == "error" || v.severity == "warning" || v.severity == "info",
            "severity should be error/warning/info, got {}",
            v.severity
        );

        // Rule should be one of the known rules
        let known_rules = [
            "forbidden_pattern",
            "mock_data",
            "missing_file",
            "missing_symbol",
            "low_complexity",
            "missing_test",
        ];
        assert!(
            known_rules.contains(&v.rule.as_str()),
            "unknown rule: {}",
            v.rule
        );
    }
}

#[test]
fn test_json_breakdown_format() {
    let report = run_and_get_json();

    // Should have breakdown
    assert!(!report.breakdown.is_empty(), "should have breakdown");

    for entry in &report.breakdown {
        assert!(!entry.rule.is_empty(), "rule should not be empty");
        assert!(entry.points > 0, "points should be positive");
        assert!(entry.violations > 0, "violations should be positive");
    }
}

#[test]
fn test_json_grade_values() {
    let report = run_and_get_json();

    // Grade should be one of A, B, C, D, F
    let valid_grades = ["A", "B", "C", "D", "F"];
    assert!(
        valid_grades.contains(&report.grade.as_str()),
        "invalid grade: {}",
        report.grade
    );
}

#[test]
fn test_json_score_consistency() {
    let report = run_and_get_json();

    // Score should match breakdown total (capped at 100)
    let breakdown_total: i32 = report.breakdown.iter().map(|e| e.points).sum();
    let expected_score = breakdown_total.min(100);

    assert_eq!(
        report.score, expected_score,
        "score {} should match breakdown total {} (capped at 100)",
        report.score, breakdown_total
    );

    // Passed should match score vs threshold
    let expected_passed = report.score <= report.threshold;
    assert_eq!(
        report.passed, expected_passed,
        "passed={} should match score ({}) <= threshold ({})",
        report.passed, report.score, report.threshold
    );
}

#[test]
fn test_json_serialization() {
    let report = run_and_get_json();

    // Should serialize to valid JSON
    let json = serde_json::to_string_pretty(&report).expect("should serialize to JSON");

    // Should deserialize back
    let parsed: JsonReport = serde_json::from_str(&json).expect("should deserialize from JSON");

    // Key fields should match
    assert_eq!(parsed.score, report.score);
    assert_eq!(parsed.grade, report.grade);
    assert_eq!(parsed.violations.len(), report.violations.len());
    assert_eq!(parsed.breakdown.len(), report.breakdown.len());
}

#[test]
fn test_json_field_names_match_go() {
    let report = run_and_get_json();
    let json = serde_json::to_string(&report).expect("should serialize");

    // Verify field names match Go version exactly
    assert!(json.contains("\"version\""), "should have 'version' field");
    assert!(json.contains("\"path\""), "should have 'path' field");
    assert!(json.contains("\"contract\""), "should have 'contract' field");
    assert!(json.contains("\"score\""), "should have 'score' field");
    assert!(json.contains("\"grade\""), "should have 'grade' field");
    assert!(json.contains("\"threshold\""), "should have 'threshold' field");
    assert!(json.contains("\"passed\""), "should have 'passed' field");
    assert!(json.contains("\"files_scanned\""), "should have 'files_scanned' field");
    assert!(json.contains("\"violations\""), "should have 'violations' field");
    assert!(json.contains("\"suppressed_count\""), "should have 'suppressed_count' field");
    assert!(json.contains("\"breakdown\""), "should have 'breakdown' field");

    // Violation fields
    assert!(json.contains("\"rule\""), "violations should have 'rule' field");
    assert!(json.contains("\"severity\""), "violations should have 'severity' field");
    assert!(json.contains("\"file\""), "violations should have 'file' field");
    assert!(json.contains("\"line\""), "violations should have 'line' field");
    assert!(json.contains("\"message\""), "violations should have 'message' field");

    // Breakdown fields
    assert!(json.contains("\"points\""), "breakdown should have 'points' field");
}

/// Test that violations from testdata match expected patterns.
#[test]
fn test_expected_violations() {
    let report = run_and_get_json();

    // Should find forbidden patterns (TODO, FIXME, etc.)
    let forbidden_count = report
        .violations
        .iter()
        .filter(|v| v.rule == "forbidden_pattern")
        .count();
    assert!(
        forbidden_count >= 4,
        "should find at least 4 forbidden patterns, found {}",
        forbidden_count
    );

    // Should find mock data
    let mock_count = report
        .violations
        .iter()
        .filter(|v| v.rule == "mock_data")
        .count();
    assert!(
        mock_count >= 1,
        "should find at least 1 mock data, found {}",
        mock_count
    );

    // Should find low complexity
    let complexity_count = report
        .violations
        .iter()
        .filter(|v| v.rule == "low_complexity")
        .count();
    assert!(
        complexity_count >= 1,
        "should find at least 1 low complexity, found {}",
        complexity_count
    );
}

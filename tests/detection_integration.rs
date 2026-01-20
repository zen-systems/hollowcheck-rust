//! Integration tests for the full detection pipeline.
//!
//! These tests validate that the detection engine correctly identifies
//! violations when run against the testdata fixtures.

use std::path::PathBuf;

use hollowcheck::contract::Contract;
use hollowcheck::detect::{Runner, ViolationRule};
use hollowcheck::parser;
use hollowcheck::score;

fn testdata_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

fn setup() {
    parser::init();
}

/// Load the test contract and run detection against testdata.
fn run_detection() -> (
    hollowcheck::detect::DetectionResult,
    hollowcheck::score::HollownessScore,
) {
    setup();

    let testdata = testdata_path();
    let contract_path = testdata.join("test-contract.yaml");
    let contract = Contract::parse_file(&contract_path).expect("should parse contract");

    // Collect all Go files in testdata
    let files: Vec<PathBuf> = std::fs::read_dir(&testdata)
        .expect("should read testdata dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "go").unwrap_or(false))
        .collect();

    let runner = Runner::new(&testdata);
    let result = runner
        .run(&files, &contract)
        .expect("detection should succeed");
    let hollowness = score::calculate(&result, &contract);

    (result, hollowness)
}

#[test]
fn test_detection_finds_forbidden_patterns() {
    let (result, _) = run_detection();

    // stub.go contains TODO, FIXME, HACK, XXX comments
    let forbidden_patterns: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.rule == ViolationRule::ForbiddenPattern)
        .collect();

    assert!(
        !forbidden_patterns.is_empty(),
        "Should find forbidden patterns in stub.go"
    );

    // Check for specific patterns
    let has_todo = forbidden_patterns
        .iter()
        .any(|v| v.message.contains("TODO"));
    let has_fixme = forbidden_patterns
        .iter()
        .any(|v| v.message.contains("FIXME"));
    let has_hack = forbidden_patterns
        .iter()
        .any(|v| v.message.contains("HACK"));
    let has_xxx = forbidden_patterns.iter().any(|v| v.message.contains("XXX"));

    assert!(has_todo, "Should find TODO pattern");
    assert!(has_fixme, "Should find FIXME pattern");
    assert!(has_hack, "Should find HACK pattern");
    assert!(has_xxx, "Should find XXX pattern");
}

#[test]
fn test_detection_finds_mock_data() {
    let (result, _) = run_detection();

    // mock.go contains mock data: example.com, 12345, 00000, lorem ipsum
    let mock_violations: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.rule == ViolationRule::MockData)
        .collect();

    assert!(
        !mock_violations.is_empty(),
        "Should find mock data in mock.go"
    );

    // Check for specific mock patterns
    let has_example_domain = mock_violations
        .iter()
        .any(|v| v.message.contains("example") && v.message.contains("com"));
    let has_fake_id = mock_violations.iter().any(|v| {
        v.message.contains("12345") || v.message.contains("00000") || v.message.contains("11111")
    });
    let has_lorem = mock_violations
        .iter()
        .any(|v| v.message.contains("lorem") && v.message.contains("ipsum"));

    assert!(has_example_domain, "Should find example.com mock domain");
    assert!(has_fake_id, "Should find fake IDs");
    assert!(has_lorem, "Should find lorem ipsum placeholder");
}

#[test]
fn test_detection_finds_low_complexity() {
    let (result, _) = run_detection();

    // ProcessData in stub.go has very low complexity (just returns nil)
    let complexity_violations: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.rule == ViolationRule::LowComplexity)
        .collect();

    assert!(
        !complexity_violations.is_empty(),
        "Should find low complexity violations"
    );

    // ProcessData should be flagged (complexity 1, required 3)
    let has_process_data = complexity_violations
        .iter()
        .any(|v| v.message.contains("ProcessData"));

    assert!(
        has_process_data,
        "ProcessData should be flagged for low complexity"
    );
}

#[test]
fn test_detection_no_missing_files() {
    let (result, _) = run_detection();

    // All required files (stub.go, clean.go) exist in testdata
    let missing_files: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.rule == ViolationRule::MissingFile)
        .collect();

    assert!(
        missing_files.is_empty(),
        "Should not have missing file violations: {:?}",
        missing_files
    );
}

#[test]
fn test_detection_no_missing_symbols() {
    let (result, _) = run_detection();

    // ProcessData and ProcessItems are both defined
    let missing_symbols: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.rule == ViolationRule::MissingSymbol)
        .collect();

    assert!(
        missing_symbols.is_empty(),
        "Should not have missing symbol violations: {:?}",
        missing_symbols
    );
}

#[test]
fn test_score_calculation() {
    let (result, hollowness) = run_detection();

    // Should have violations
    assert!(
        !result.violations.is_empty(),
        "Should have found some violations"
    );

    // Score should be positive (have violations)
    assert!(
        hollowness.score > 0,
        "Score should be > 0 with violations, got {}",
        hollowness.score
    );

    // Score should have a valid grade
    assert!(
        ["A", "B", "C", "D", "F"].contains(&hollowness.grade.as_str()),
        "Grade should be A-F, got {}",
        hollowness.grade
    );

    // Breakdown should have entries
    assert!(
        !hollowness.breakdown.is_empty(),
        "Breakdown should have entries"
    );
}

#[test]
fn test_score_breakdown_matches_violations() {
    let (result, hollowness) = run_detection();

    // Count violations by rule
    let mut expected_breakdown = std::collections::HashMap::new();
    for v in &result.violations {
        let rule_str = v.rule.as_str().to_string();
        let points = match v.rule {
            ViolationRule::MissingFile => 20,
            ViolationRule::MissingSymbol => 15,
            ViolationRule::ForbiddenPattern => 10,
            ViolationRule::LowComplexity => 10,
            ViolationRule::MissingTest => 5,
            ViolationRule::MockData => 3,
            _ => 0,
        };
        *expected_breakdown.entry(rule_str).or_insert(0) += points;
    }

    // Verify breakdown matches
    for (rule, expected_points) in &expected_breakdown {
        let actual = hollowness.breakdown.get(rule).copied().unwrap_or(0);
        assert_eq!(
            actual, *expected_points,
            "Breakdown mismatch for {}: expected {}, got {}",
            rule, expected_points, actual
        );
    }
}

#[test]
fn test_detection_with_real_files() {
    setup();

    let testdata = testdata_path();

    // Verify testdata files exist
    assert!(
        testdata.join("clean.go").exists(),
        "clean.go should exist in testdata"
    );
    assert!(
        testdata.join("stub.go").exists(),
        "stub.go should exist in testdata"
    );
    assert!(
        testdata.join("mock.go").exists(),
        "mock.go should exist in testdata"
    );
    assert!(
        testdata.join("test-contract.yaml").exists(),
        "test-contract.yaml should exist in testdata"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_tree_sitter_symbol_extraction_on_clean_go() {
    setup();

    let testdata = testdata_path();
    let clean_go = testdata.join("clean.go");

    let parser = hollowcheck::parser::for_extension(".go").expect("Go parser should be available");
    let source = std::fs::read(&clean_go).expect("should read clean.go");
    let symbols = parser.parse_symbols(&source).expect("should parse symbols");

    // Verify expected symbols
    let symbol_names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(
        symbol_names.contains(&"Config"),
        "Should find Config type, found: {:?}",
        symbol_names
    );
    assert!(
        symbol_names.contains(&"Validate"),
        "Should find Validate method"
    );
    assert!(
        symbol_names.contains(&"ProcessItems"),
        "Should find ProcessItems function"
    );
    assert!(
        symbol_names.contains(&"CalculateScore"),
        "Should find CalculateScore function"
    );
    assert!(
        symbol_names.contains(&"MaxConnections"),
        "Should find MaxConnections const"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_tree_sitter_complexity_on_clean_go() {
    setup();

    let testdata = testdata_path();
    let clean_go = testdata.join("clean.go");

    let parser = hollowcheck::parser::for_extension(".go").expect("Go parser should be available");
    let source = std::fs::read(&clean_go).expect("should read clean.go");

    // ProcessItems has multiple if statements, a for loop, and an && operator
    let process_items_cc = parser
        .complexity(&source, "ProcessItems")
        .expect("should calculate complexity");

    assert!(
        process_items_cc >= 5,
        "ProcessItems should have complexity >= 5, got {}",
        process_items_cc
    );

    // Validate has multiple if statements
    let validate_cc = parser
        .complexity(&source, "Validate")
        .expect("should calculate complexity");

    assert!(
        validate_cc >= 4,
        "Validate should have complexity >= 4, got {}",
        validate_cc
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_tree_sitter_complexity_on_stub_go() {
    setup();

    let testdata = testdata_path();
    let stub_go = testdata.join("stub.go");

    let parser = hollowcheck::parser::for_extension(".go").expect("Go parser should be available");
    let source = std::fs::read(&stub_go).expect("should read stub.go");

    // ProcessData is a stub with just `return nil` - complexity 1
    let process_data_cc = parser
        .complexity(&source, "ProcessData")
        .expect("should calculate complexity");

    assert!(
        process_data_cc < 3,
        "ProcessData (stub) should have complexity < 3, got {}",
        process_data_cc
    );
}

/// Verify the scoring matches expected ranges based on the testdata contract.
#[test]
fn test_expected_score_range() {
    let (result, hollowness) = run_detection();

    // Expected violations:
    // - 4 forbidden patterns (TODO, FIXME, HACK, XXX) = 40 points (Warning - doesn't count toward score)
    // - Multiple mock data signatures = varies (Warning - doesn't count toward score)
    // - 1 low complexity (ProcessData) = 10 points (Error - counts toward score)
    //
    // Only Critical and Error severities count toward the score.
    // ForbiddenPattern and MockData are now Warning severity.

    println!("Total violations: {}", result.violations.len());
    println!("Score: {} (Grade: {})", hollowness.score, hollowness.grade);
    println!("Breakdown: {:?}", hollowness.breakdown);

    // Score should include at least the low_complexity violation (10 points)
    assert!(
        hollowness.score >= 10,
        "Score should be >= 10 with the low_complexity violation, got {}",
        hollowness.score
    );

    // With only Error severity violations counting, score should be 10 (low_complexity)
    // which is less than the default threshold of 25, so it should pass
    assert!(
        hollowness.passed,
        "Should pass with default threshold since only Error/Critical count, score={} threshold={}",
        hollowness.score, hollowness.threshold
    );
}

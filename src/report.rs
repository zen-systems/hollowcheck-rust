// hollowcheck:ignore-file mock_data - Documentation describes mock patterns
//! Output formatting for hollowcheck results.
//!
//! Supports three output formats:
//! - Pretty: colored terminal output for human readability
//! - JSON: structured output for programmatic consumption (matches Go version)
//! - SARIF: Static Analysis Results Interchange Format for IDE/CI integration

use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::detect::{DetectionResult, Severity, SuppressedViolation, Violation};
use crate::score::HollownessScore;

// =============================================================================
// JSON Format (matches Go version exactly)
// =============================================================================

/// JSON report structure matching Go's JSONReport.
#[derive(Serialize, Deserialize)]
pub struct JsonReport {
    pub version: String,
    pub path: String,
    pub contract: String,
    pub score: i32,
    pub grade: String,
    pub threshold: i32,
    pub passed: bool,
    pub files_scanned: usize,
    pub violations: Vec<JsonViolation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_violations: Vec<JsonViolation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed: Vec<JsonSuppressedViolation>,
    pub suppressed_count: usize,
    pub breakdown: Vec<BreakdownEntry>,
}

/// JSON violation structure matching Go's JSONViolation.
#[derive(Serialize, Deserialize)]
pub struct JsonViolation {
    pub rule: String,
    pub severity: String,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Breakdown entry for score details.
#[derive(Serialize, Deserialize)]
pub struct BreakdownEntry {
    pub rule: String,
    pub points: i32,
    pub violations: i32,
}

/// Suppressed violation with suppression info.
#[derive(Serialize, Deserialize)]
pub struct JsonSuppressedViolation {
    pub violation: JsonViolation,
    pub suppression: JsonSuppression,
}

/// Suppression directive info.
#[derive(Serialize, Deserialize)]
pub struct JsonSuppression {
    pub rule: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub file: String,
    pub line: usize,
    #[serde(rename = "type")]
    pub suppression_type: String,
}

/// Write results in JSON format (matches Go version exactly).
pub fn write_json(
    path: &str,
    contract_path: &str,
    result: &DetectionResult,
    score: &HollownessScore,
) -> anyhow::Result<()> {
    let violations: Vec<JsonViolation> = result.violations.iter().map(violation_to_json).collect();

    let new_violations: Vec<JsonViolation> = result
        .new_violations
        .iter()
        .map(violation_to_json)
        .collect();

    let suppressed: Vec<JsonSuppressedViolation> = result
        .suppressed
        .iter()
        .map(|sv| JsonSuppressedViolation {
            violation: violation_to_json(&sv.violation),
            suppression: JsonSuppression {
                rule: sv.suppression.rule.clone(),
                reason: sv.suppression.reason.clone(),
                file: sv.suppression.file.clone(),
                line: sv.suppression.line,
                suppression_type: format!("{:?}", sv.suppression.suppression_type).to_lowercase(),
            },
        })
        .collect();

    // Build breakdown with violation counts
    let breakdown: Vec<BreakdownEntry> = score
        .breakdown
        .iter()
        .map(|(rule, points)| BreakdownEntry {
            rule: rule.clone(),
            points: *points,
            violations: score.violation_count(rule),
        })
        .collect();

    let report = JsonReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        path: path.to_string(),
        contract: contract_path.to_string(),
        score: score.score,
        grade: score.grade.clone(),
        threshold: score.threshold,
        passed: score.passed,
        files_scanned: result.scanned,
        violations,
        new_violations,
        baseline_ref: result.baseline_ref.clone(),
        suppressed,
        suppressed_count: result.suppressed.len(),
        breakdown,
    };

    let json = serde_json::to_string_pretty(&report)?;
    println!("{}", json);
    Ok(())
}

fn violation_to_json(v: &Violation) -> JsonViolation {
    JsonViolation {
        rule: v.rule.as_str().to_string(),
        severity: v.severity.to_string(),
        file: v.file.clone(),
        line: v.line,
        message: v.message.clone(),
    }
}

// =============================================================================
// SARIF Format (matches Go version exactly)
// =============================================================================

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";
const TOOL_NAME: &str = "hollowcheck";
const INFO_URI: &str = "https://github.com/zen-systems/hollowcheck";

#[derive(Serialize, Deserialize)]
struct SarifReport {
    version: String,
    #[serde(rename = "$schema")]
    schema: String,
    runs: Vec<SarifRun>,
}

#[derive(Serialize, Deserialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize, Deserialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize, Deserialize)]
struct SarifDriver {
    name: String,
    version: String,
    #[serde(rename = "informationUri")]
    information_uri: String,
    rules: Vec<SarifRule>,
}

#[derive(Serialize, Deserialize)]
struct SarifRule {
    id: String,
    name: String,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
    #[serde(rename = "fullDescription", skip_serializing_if = "Option::is_none")]
    full_description: Option<SarifMessage>,
    #[serde(rename = "helpUri", skip_serializing_if = "Option::is_none")]
    help_uri: Option<String>,
    #[serde(rename = "defaultConfiguration")]
    default_config: SarifRuleConfig,
}

#[derive(Serialize, Deserialize)]
struct SarifRuleConfig {
    level: String,
}

#[derive(Serialize, Deserialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize, Deserialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize, Deserialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize, Deserialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifact,
    region: SarifRegion,
}

#[derive(Serialize, Deserialize)]
struct SarifArtifact {
    uri: String,
}

#[derive(Serialize, Deserialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: usize,
}

/// Rule metadata for SARIF output.
struct RuleInfo {
    name: &'static str,
    short_description: &'static str,
    full_description: &'static str,
    help_uri: &'static str,
    default_level: &'static str,
}

fn get_rule_info(rule_id: &str) -> RuleInfo {
    match rule_id {
        "forbidden_pattern" => RuleInfo {
            name: "ForbiddenPattern",
            short_description: "Detects forbidden patterns like TODO, FIXME, panic(\"not implemented\")",
            full_description: "Identifies code patterns that indicate incomplete or placeholder implementations, such as TODO comments, FIXME markers, and panic statements.",
            help_uri: "#forbidden-patterns",
            default_level: "error",
        },
        "mock_data" => RuleInfo {
            name: "MockData",
            short_description: "Detects mock/placeholder data like example.com, fake IDs",
            full_description: "Identifies hardcoded placeholder values that should be replaced with real data or configuration, such as example.com domains, fake UUIDs, and test credentials.",
            help_uri: "#mock-data",
            default_level: "warning",
        },
        "missing_file" => RuleInfo {
            name: "MissingFile",
            short_description: "Detects missing required files",
            full_description: "Verifies that all files specified as required in the contract exist in the project.",
            help_uri: "#required-files",
            default_level: "error",
        },
        "missing_symbol" => RuleInfo {
            name: "MissingSymbol",
            short_description: "Detects missing required symbols (functions, types)",
            full_description: "Verifies that all symbols (functions, types, constants) specified as required in the contract are defined in the code.",
            help_uri: "#required-symbols",
            default_level: "error",
        },
        "low_complexity" => RuleInfo {
            name: "LowComplexity",
            short_description: "Detects stub implementations with suspiciously low complexity",
            full_description: "Identifies functions that have cyclomatic complexity below the expected threshold, suggesting they may be stub or placeholder implementations.",
            help_uri: "#complexity",
            default_level: "error",
        },
        "missing_test" => RuleInfo {
            name: "MissingTest",
            short_description: "Detects missing required test functions",
            full_description: "Verifies that all test functions specified as required in the contract exist.",
            help_uri: "#required-tests",
            default_level: "warning",
        },
        "hallucinated_dependency" => RuleInfo {
            name: "HallucinatedDependency",
            short_description: "Detects imports of packages that don't exist in public registries",
            full_description: "Identifies code that imports packages which cannot be found in their respective package registries (PyPI, npm, crates.io, Go proxy), suggesting the code may be AI-generated with hallucinated dependencies.",
            help_uri: "#hallucinated-dependencies",
            default_level: "error",
        },
        // Prose rules
        "filler_phrase" => RuleInfo {
            name: "FillerPhrase",
            short_description: "Detects filler phrases that add no meaning",
            full_description: "Identifies redundant phrases, hedging language, and filler words that dilute the clarity of prose.",
            help_uri: "#prose-fillers",
            default_level: "warning",
        },
        "weasel_word" => RuleInfo {
            name: "WeaselWord",
            short_description: "Detects weasel words and vague language",
            full_description: "Identifies anonymous authority claims, passive voice constructions, and other language patterns that reduce precision.",
            help_uri: "#prose-weasels",
            default_level: "warning",
        },
        "low_density" => RuleInfo {
            name: "LowDensity",
            short_description: "Detects sections with low information density",
            full_description: "Identifies text sections that have a low ratio of content words to total words, suggesting padding or filler content.",
            help_uri: "#prose-density",
            default_level: "warning",
        },
        "prose_repetitive_opener" => RuleInfo {
            name: "RepetitiveOpener",
            short_description: "Detects repetitive sentence openers",
            full_description: "Identifies when multiple sentences start with the same pattern, suggesting formulaic or AI-generated prose.",
            help_uri: "#prose-structure",
            default_level: "warning",
        },
        "prose_middle_sag" => RuleInfo {
            name: "MiddleSag",
            short_description: "Detects middle sections with lower quality than intro/conclusion",
            full_description: "Identifies when the middle of a document has significantly lower information density than the introduction and conclusion.",
            help_uri: "#prose-structure",
            default_level: "error",
        },
        "prose_weak_transition" => RuleInfo {
            name: "WeakTransition",
            short_description: "Detects weak sentence transitions",
            full_description: "Identifies sentences that start with weak transitional phrases like 'And', 'But', 'So' at the beginning.",
            help_uri: "#prose-structure",
            default_level: "note",
        },
        _ => {
            // Default for unknown rules - convert snake_case to PascalCase
            RuleInfo {
                name: "Unknown",
                short_description: "Unknown rule type",
                full_description: "An unknown violation was detected.",
                help_uri: "",
                default_level: "warning",
            }
        }
    }
}

fn map_severity_to_level(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn make_relative_path(file_path: &str, base_path: &Path) -> String {
    if base_path.to_string_lossy().is_empty() {
        return file_path.to_string();
    }

    let file = Path::new(file_path);

    // If they're the same (single file scan), return just the filename
    if file == base_path {
        return file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string());
    }

    file.strip_prefix(base_path)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file_path.to_string())
}

/// Write results in SARIF format.
pub fn write_sarif(base_path: &Path, result: &DetectionResult) -> anyhow::Result<()> {
    // Collect unique rules from violations
    let rule_set: HashSet<String> = result
        .violations
        .iter()
        .map(|v| v.rule.as_str().to_string())
        .collect();

    // Build rules list
    let rules: Vec<SarifRule> = rule_set
        .iter()
        .map(|rule_id| {
            let info = get_rule_info(rule_id);
            SarifRule {
                id: rule_id.clone(),
                name: info.name.to_string(),
                short_description: SarifMessage {
                    text: info.short_description.to_string(),
                },
                full_description: Some(SarifMessage {
                    text: info.full_description.to_string(),
                }),
                help_uri: if info.help_uri.is_empty() {
                    Some(INFO_URI.to_string())
                } else {
                    Some(format!("{}{}", INFO_URI, info.help_uri))
                },
                default_config: SarifRuleConfig {
                    level: info.default_level.to_string(),
                },
            }
        })
        .collect();

    // Build results list
    let results: Vec<SarifResult> = result
        .violations
        .iter()
        .map(|v| SarifResult {
            rule_id: v.rule.as_str().to_string(),
            level: map_severity_to_level(&v.severity).to_string(),
            message: SarifMessage {
                text: v.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifact {
                        uri: make_relative_path(&v.file, base_path),
                    },
                    region: SarifRegion {
                        start_line: if v.line > 0 { v.line } else { 1 },
                    },
                },
            }],
        })
        .collect();

    let report = SarifReport {
        version: SARIF_VERSION.to_string(),
        schema: SARIF_SCHEMA.to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: TOOL_NAME.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: INFO_URI.to_string(),
                    rules,
                },
            },
            results,
        }],
    };

    let json = serde_json::to_string_pretty(&report)?;
    println!("{}", json);
    Ok(())
}

// =============================================================================
// Pretty Format (matches Go version's visual style)
// =============================================================================

/// Write results in pretty (human-readable) format.
pub fn write_pretty(
    path: &str,
    contract_path: &str,
    result: &DetectionResult,
    score: &HollownessScore,
    show_suppressed: bool,
) {
    // Header
    println!();
    print!("  ");
    print!("{}", "hollowcheck".cyan().bold());
    println!(" v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // Scan info
    print!("  {}", "Scanning: ".dimmed());
    println!("{}", path);
    print!("  {}", "Contract: ".dimmed());
    println!("{}", contract_path);

    // Show baseline ref if in baseline mode
    if let Some(ref baseline) = result.baseline_ref {
        print!("  {}", "Baseline: ".dimmed());
        println!("{}", baseline);
    }
    println!();

    // Result summary
    write_result_summary(score, result.suppressed.len());
    println!();

    // Violations
    if !result.violations.is_empty() {
        write_violations(&result.violations);
        println!();
    }

    // Suppressed violations
    if !result.suppressed.is_empty() {
        write_suppressed_summary(&result.suppressed, show_suppressed);
        println!();
    }

    // Breakdown
    if !score.breakdown.is_empty() {
        write_breakdown(score);
        println!();
    }

    // Final status line
    write_final_status(score);
    println!();
}

fn write_result_summary(score: &HollownessScore, suppressed_count: usize) {
    if score.passed {
        print!("  {}", "✓ PASS".green());
    } else {
        print!("  {}", "✗ FAIL".red());
    }

    print!("  Hollowness: ");
    write_colored_score(score.score);
    print!("%  Grade: ");
    write_colored_grade(&score.grade);

    if suppressed_count > 0 {
        print!(
            "  {}",
            format!("({} suppressed)", suppressed_count).dimmed()
        );
    }

    println!();
}

fn write_colored_score(s: i32) {
    match s {
        s if s <= 10 => print!("{}", s.to_string().green().bold()),
        s if s <= 25 => print!("{}", s.to_string().green()),
        s if s <= 50 => print!("{}", s.to_string().yellow()),
        s if s <= 75 => print!("{}", s.to_string().yellow().bold()),
        _ => print!("{}", s.to_string().red()),
    }
}

fn write_colored_grade(grade: &str) {
    match grade {
        "A" => print!("{}", grade.green().bold()),
        "B" => print!("{}", grade.green()),
        "C" => print!("{}", grade.yellow()),
        "D" => print!("{}", grade.yellow().bold()),
        _ => print!("{}", grade.red()),
    }
}

fn write_violations(violations: &[Violation]) {
    println!("  {} ({}):", "Violations".bold(), violations.len());
    println!();

    for v in violations {
        write_severity_tag(&v.severity);
        print!("   ");
        print!("{:<18}", v.rule.as_str().dimmed());
        print!("{}", v.file.blue());
        if v.line > 0 {
            print!("{}", format!(":{}", v.line).dimmed());
        }
        println!();

        // Message on next line, indented
        println!("            {}", v.message);
        println!();
    }
}

fn write_severity_tag(severity: &Severity) {
    match severity {
        Severity::Error => print!("    {} ", "ERROR".red()),
        Severity::Warning => print!("    {} ", "WARN ".yellow()),
        Severity::Info => print!("    {} ", "INFO ".blue()),
    }
}

fn write_breakdown(score: &HollownessScore) {
    println!("  {}", "Breakdown:".bold());

    // Sort rules by points descending
    let mut rules: Vec<(&String, &i32)> = score.breakdown.iter().collect();
    rules.sort_by(|a, b| b.1.cmp(a.1));

    for (rule, points) in rules {
        let count = score.violation_count(rule);
        let plural = if count != 1 { "s" } else { "" };
        println!(
            "    {:<20} {:>3} pts ({} violation{})",
            rule, points, count, plural
        );
    }
}

fn write_final_status(score: &HollownessScore) {
    print!("  {}", format!("Threshold: {}", score.threshold).dimmed());
    print!("  Score: ");
    write_colored_score(score.score);
    print!("  ");

    if score.passed {
        print!("{}", "PASSED".green());
    } else {
        print!("{}", "FAILED".red());
    }
    println!();
}

fn write_suppressed_summary(suppressed: &[SuppressedViolation], show_details: bool) {
    println!("  {} ({}):", "Suppressed".dimmed(), suppressed.len());

    if !show_details {
        println!("    {}", "(use --show-suppressed to see details)".dimmed());
        return;
    }

    println!();
    for sv in suppressed {
        let v = &sv.violation;
        let s = &sv.suppression;

        print!("    {:<18}", v.rule.as_str().dimmed());
        print!("{}", v.file.blue());
        if matches!(s.suppression_type, crate::detect::SuppressionType::File) {
            print!("{}", ":* (file)".dimmed());
        } else if v.line > 0 {
            print!("{}", format!(":{}", v.line).dimmed());
        }
        println!();

        if !s.reason.is_empty() {
            println!("            {}", format!("reason: {:?}", s.reason).dimmed());
        }
    }
}

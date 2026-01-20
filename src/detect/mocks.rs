// hollowcheck:ignore-file mock_data - Test fixtures contain mock patterns
//! Detection of mock data signatures in code.

use crate::contract::MockSignaturesConfig;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Pre-compiled mock signature with metadata.
struct CompiledMockSignature {
    regex: Regex,
    description: Option<String>,
}

/// Check if a file path is a test file (ends with _test.go).
fn is_test_file(file_path: &Path) -> bool {
    file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.ends_with("_test.go"))
        .unwrap_or(false)
}

/// Check if a file is a known utility that legitimately contains mock-like patterns.
/// These files are intentionally designed to contain placeholder/example data.
fn is_known_utility_file(file_path: &Path) -> bool {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Files that legitimately contain "lorem ipsum" or placeholder text
    // Only match specific known utility file names, not directories
    if file_name == "lorem_ipsum.py" || file_name == "lorem.py" ||
       file_name.starts_with("lorem") {
        return true;
    }

    false
}

/// Scan files for mock data signatures defined in the contract.
pub fn detect_mock_data<P: AsRef<Path>>(
    files: &[P],
    cfg: Option<&MockSignaturesConfig>,
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    let cfg = match cfg {
        Some(c) if !c.patterns.is_empty() => c,
        _ => return Ok(result),
    };

    // Pre-compile all patterns
    let compiled: Vec<CompiledMockSignature> = cfg
        .patterns
        .iter()
        .map(|s| {
            let regex = Regex::new(&s.pattern)
                .map_err(|e| anyhow::anyhow!("compiling mock signature {:?}: {}", s.pattern, e))?;
            Ok(CompiledMockSignature {
                regex,
                description: s.description.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Determine test file handling
    let skip_test_files = cfg.should_skip_test_files();
    let test_file_severity = cfg.get_test_file_severity();

    // Scan each file
    for file in files {
        let path = file.as_ref();
        let is_test = is_test_file(path);

        // Skip known utility files that legitimately contain mock-like patterns
        if is_known_utility_file(path) {
            result.scanned += 1;
            continue;
        }

        // Skip test files if configured
        if is_test && skip_test_files {
            result.scanned += 1;
            continue;
        }

        // Determine severity for this file
        let severity = if is_test {
            match test_file_severity {
                Some("info") => Severity::Info,
                Some("warning") => Severity::Warning,
                Some("error") => Severity::Error,
                _ => Severity::Warning,
            }
        } else {
            Severity::Warning
        };

        let violations = scan_file_for_mocks(path, &compiled, severity)?;
        result.violations.extend(violations);
        result.scanned += 1;
    }

    Ok(result)
}

/// Check if the line context suggests this is legitimate configuration data,
/// not mock/placeholder data.
fn is_legitimate_context(line: &str, surrounding_lines: &[String], line_idx: usize) -> bool {
    let line_lower = line.to_lowercase();

    // Skip if it's defining ranges, limits, or bounds (common for numeric constants)
    let config_keywords = [
        "range", "limit", "max", "min", "bound", "field_range", "precision",
        "scale", "constraint", "schema", "oracle", "decimal", "numeric",
        "integer", "float", "double", "bigint", "constant", "config",
    ];
    if config_keywords.iter().any(|kw| line_lower.contains(kw)) {
        return true;
    }

    // Skip if the line contains mathematical/schema operators suggesting numeric bounds
    // e.g., (-999999999999999, 999999999999999) or MIN_VALUE, MAX_VALUE
    if line.contains("MIN") || line.contains("MAX") || line.contains("LIMIT") {
        return true;
    }

    // Skip if the line contains arithmetic operations with large numbers
    // e.g., "* 1000000" for microsecond conversions, "/ 1000000" for unit conversions
    if line.contains("* 1") || line.contains("/ 1") || line.contains("% 1") {
        // Check if followed by zeros (common multipliers like 1000, 1000000, etc.)
        let multiplier_pattern = regex::Regex::new(r"[*/]\s*10+\b").unwrap();
        if multiplier_pattern.is_match(line) {
            return true;
        }
    }

    // Skip SQL type casts (AS SIGNED, AS INTEGER, AS DECIMAL, etc.)
    if line_lower.contains(" as ") &&
       (line_lower.contains("signed") || line_lower.contains("unsigned") ||
        line_lower.contains("integer") || line_lower.contains("decimal")) {
        return true;
    }

    // Skip time/duration calculations (common in database backends)
    if line_lower.contains("time") || line_lower.contains("second") ||
       line_lower.contains("micro") || line_lower.contains("milli") ||
       line_lower.contains("duration") {
        return true;
    }

    // Skip character set definitions (e.g., "0123456789abcdef" for hex, base64, etc.)
    // These contain sequential digits as part of a legitimate charset
    let charset_pattern = regex::Regex::new(r#"["'][0-9a-zA-Z!@#$%^&*()_+=\-]{15,}["']"#).unwrap();
    if charset_pattern.is_match(line) {
        return true;
    }

    // Skip lines that look like charset/alphabet definitions
    if line_lower.contains("char") || line_lower.contains("alphabet") ||
       line_lower.contains("digit") || line_lower.contains("hexdig") ||
       line_lower.contains("base64") || line_lower.contains("base32") {
        return true;
    }

    // Skip numeric comparisons (e.g., "< 1000000", "> 99999")
    let comparison_pattern = regex::Regex::new(r"[<>]=?\s*-?\d{4,}").unwrap();
    if comparison_pattern.is_match(line) {
        return true;
    }

    // Skip very small decimals (scientific notation style, e.g., 0.000001)
    if line.contains("0.000") || line.contains("1e-") || line.contains("1E-") {
        return true;
    }

    // Skip numeric tuples/ranges (e.g., (-99999, 99999) for database field ranges)
    let range_tuple_pattern = regex::Regex::new(r"\(\s*-?\d+\s*,\s*-?\d+\s*\)").unwrap();
    if range_tuple_pattern.is_match(line) {
        return true;
    }

    // Skip auto/field definitions (database field type definitions)
    if line_lower.contains("autofield") || line_lower.contains("integerfield") ||
       line_lower.contains("smallint") || line_lower.contains("bigint") {
        return true;
    }

    // Skip if it looks like a tuple of numeric bounds (Python style)
    // e.g., "decimal_field_ranges": (-999999999999999999999999999999999999999, ...)
    let tuple_pattern = regex::Regex::new(r"\(\s*-?\d{10,}").unwrap();
    if tuple_pattern.is_match(line) {
        return true;
    }

    // Skip if it's in a dictionary literal context (JSON/Python dict)
    let in_dict_context = line.contains(":") && (line.contains("{") || line.contains("}"));
    if in_dict_context {
        // Check if we're in a configuration dictionary
        let context_start = line_idx.saturating_sub(5);
        let context_end = (line_idx + 3).min(surrounding_lines.len());
        for i in context_start..context_end {
            if let Some(ctx_line) = surrounding_lines.get(i) {
                let ctx_lower = ctx_line.to_lowercase();
                if ctx_lower.contains("range")
                    || ctx_lower.contains("config")
                    || ctx_lower.contains("field")
                    || ctx_lower.contains("schema")
                    || ctx_lower.contains("constraint")
                {
                    return true;
                }
            }
        }
    }

    // Skip class-level constants (ALL_CAPS naming convention)
    let trimmed = line.trim();
    if let Some(var_name) = trimmed.split(&['=', ':', ' '][..]).next() {
        let var_name = var_name.trim();
        // Check if it's an all-caps constant
        if !var_name.is_empty()
            && var_name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
        {
            return true;
        }
        // Check if variable name contains range/limit/config hints
        let var_lower = var_name.to_lowercase();
        if config_keywords.iter().any(|kw| var_lower.contains(kw)) {
            return true;
        }
    }

    // Skip if it's inside a comment (common for documentation)
    if trimmed.starts_with("//")
        || trimmed.starts_with("#")
        || trimmed.starts_with("*")
        || trimmed.starts_with("/*")
    {
        return true;
    }

    // Skip documentation patterns (RST, markdown, docstrings)
    // e.g., ``example.com``, `example.com`, :ref:`example`
    if line.contains("``") || line.contains(".. ") || line.contains(":ref:") ||
       line.contains(">>>") || line.contains("...") && line.contains("e.g.") {
        return true;
    }

    // Skip lines that are clearly documentation (contain doc-like patterns)
    if line_lower.contains("e.g.") || line_lower.contains("i.e.") ||
       line_lower.contains("for example") || line_lower.contains("such as") {
        return true;
    }

    // Skip default/example site configurations (common in web frameworks)
    if line_lower.contains("default") && line_lower.contains("site") {
        return true;
    }

    false
}

/// Scan a single file for mock data signatures.
fn scan_file_for_mocks(
    file_path: &Path,
    signatures: &[CompiledMockSignature],
    severity: Severity,
) -> anyhow::Result<Vec<Violation>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut violations = Vec::new();
    let file_str = file_path.to_string_lossy().to_string();

    // Read all lines for context awareness
    let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_number = line_idx + 1;

        // Skip if this looks like legitimate configuration data
        if is_legitimate_context(line, &lines, line_idx) {
            continue;
        }

        for s in signatures {
            if s.regex.is_match(line) {
                let msg = if let Some(desc) = &s.description {
                    format!("mock data signature {:?} found: {}", s.regex.as_str(), desc)
                } else {
                    format!("mock data signature {:?} found", s.regex.as_str())
                };

                violations.push(Violation {
                    rule: ViolationRule::MockData,
                    message: msg,
                    file: file_str.clone(),
                    line: line_number,
                    severity,
                });
            }
        }
    }

    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::MockSignature;
    use tempfile::TempDir;

    #[test]
    fn test_detect_mock_data() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        std::fs::write(
            &file_path,
            r#"
var apiUrl = "https://example.com/api"
var userId = "user-12345"
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![
                MockSignature {
                    pattern: r"example\.com".to_string(),
                    description: Some("Mock domain".to_string()),
                },
                MockSignature {
                    pattern: r"user-\d+".to_string(),
                    description: Some("Mock user ID".to_string()),
                },
            ],
            skip_test_files: None,
            test_file_severity: None,
        };

        let result = detect_mock_data(&[&file_path], Some(&cfg)).unwrap();
        assert_eq!(result.violations.len(), 2);
        assert!(result
            .violations
            .iter()
            .all(|v| v.rule == ViolationRule::MockData));
    }

    #[test]
    fn test_skip_test_files() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("main_test.go");
        std::fs::write(
            &test_file,
            r#"
var testUrl = "https://example.com/api"
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![MockSignature {
                pattern: r"example\.com".to_string(),
                description: None,
            }],
            skip_test_files: Some(true), // Default behavior
            test_file_severity: None,
        };

        let result = detect_mock_data(&[&test_file], Some(&cfg)).unwrap();
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.scanned, 1);
    }

    #[test]
    fn test_test_file_with_severity() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("main_test.go");
        std::fs::write(
            &test_file,
            r#"
var testUrl = "https://example.com/api"
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![MockSignature {
                pattern: r"example\.com".to_string(),
                description: None,
            }],
            skip_test_files: Some(false),
            test_file_severity: Some("info".to_string()),
        };

        let result = detect_mock_data(&[&test_file], Some(&cfg)).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].severity, Severity::Info);
    }

    #[test]
    fn test_skip_numeric_range_constants() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("schema.py");
        std::fs::write(
            &file_path,
            r#"
# Oracle NUMBER precision limits
integer_field_ranges = {
    "small": (-999999999, 999999999),
    "large": (-999999999999999999999999999999999999999, 999999999999999999999999999999999999999),
}

decimal_field_ranges = {
    "precision_38": (-99999999999999999999999999999999999999, 99999999999999999999999999999999999999),
}

MAX_ORACLE_NUMBER = 999999999999999999999999999999999999999
MIN_ORACLE_NUMBER = -999999999999999999999999999999999999999
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![MockSignature {
                pattern: r"9{10,}".to_string(),  // Many 9s pattern
                description: Some("Suspicious repeated digits".to_string()),
            }],
            skip_test_files: None,
            test_file_severity: None,
        };

        let result = detect_mock_data(&[&file_path], Some(&cfg)).unwrap();
        // Should NOT flag these as mock data - they're legitimate numeric constants
        assert_eq!(result.violations.len(), 0, "Should not flag numeric range constants");
    }

    #[test]
    fn test_skip_config_dictionaries() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("config.py");
        std::fs::write(
            &file_path,
            r#"
FIELD_LIMITS = {
    "max_value": 999999999999,
    "min_value": -999999999999,
}
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![MockSignature {
                pattern: r"9{10,}".to_string(),
                description: None,
            }],
            skip_test_files: None,
            test_file_severity: None,
        };

        let result = detect_mock_data(&[&file_path], Some(&cfg)).unwrap();
        assert_eq!(result.violations.len(), 0, "Should not flag config dictionary values");
    }

    #[test]
    fn test_still_flags_actual_mock_data() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("handler.go");
        std::fs::write(
            &file_path,
            r#"
func GetUser() User {
    return User{
        Email: "test@example.com",
        Phone: "555-1234",
    }
}
"#,
        )
        .unwrap();

        let cfg = MockSignaturesConfig {
            patterns: vec![
                MockSignature {
                    pattern: r"example\.com".to_string(),
                    description: Some("Mock domain".to_string()),
                },
                MockSignature {
                    pattern: r"555-\d{4}".to_string(),
                    description: Some("Mock phone".to_string()),
                },
            ],
            skip_test_files: None,
            test_file_severity: None,
        };

        let result = detect_mock_data(&[&file_path], Some(&cfg)).unwrap();
        // Should still flag actual mock data in non-config contexts
        assert_eq!(result.violations.len(), 2, "Should flag actual mock data");
    }
}

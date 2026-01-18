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

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let line_number = line_num + 1;

        for s in signatures {
            if s.regex.is_match(&line) {
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
}

//! Detection of forbidden patterns in code.

use crate::contract::ForbiddenPattern;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Pre-compiled pattern with metadata.
struct CompiledPattern {
    regex: Regex,
    description: Option<String>,
}

/// Scan files for forbidden patterns defined in the contract.
pub fn detect_forbidden_patterns<P: AsRef<Path>>(
    files: &[P],
    patterns: &[ForbiddenPattern],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    if patterns.is_empty() {
        return Ok(result);
    }

    // Pre-compile all patterns
    let compiled: Vec<CompiledPattern> = patterns
        .iter()
        .map(|p| {
            let regex = Regex::new(&p.pattern)
                .map_err(|e| anyhow::anyhow!("compiling pattern {:?}: {}", p.pattern, e))?;
            Ok(CompiledPattern {
                regex,
                description: p.description.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Scan each file
    for file in files {
        let violations = scan_file_for_patterns(file.as_ref(), &compiled)?;
        result.violations.extend(violations);
        result.scanned += 1;
    }

    Ok(result)
}

/// Scan a single file for forbidden patterns.
fn scan_file_for_patterns(
    file_path: &Path,
    patterns: &[CompiledPattern],
) -> anyhow::Result<Vec<Violation>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut violations = Vec::new();
    let file_str = file_path.to_string_lossy().to_string();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let line_number = line_num + 1;

        for p in patterns {
            // Find all matches with their positions
            for mat in p.regex.find_iter(&line) {
                // Skip if match is inside a string literal
                if is_inside_string_literal(&line, mat.start()) {
                    continue;
                }

                let msg = if let Some(desc) = &p.description {
                    format!("forbidden pattern {:?} found: {}", p.regex.as_str(), desc)
                } else {
                    format!("forbidden pattern {:?} found", p.regex.as_str())
                };

                violations.push(Violation {
                    rule: ViolationRule::ForbiddenPattern,
                    message: msg,
                    file: file_str.clone(),
                    line: line_number,
                    severity: Severity::Error,
                });
            }
        }
    }

    Ok(violations)
}

/// Check if a position in a line falls within a string literal.
/// Supports double-quoted, single-quoted, and backtick strings with escape handling.
fn is_inside_string_literal(line: &str, pos: usize) -> bool {
    let mut in_string = false;
    let mut string_char = None;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if i >= pos {
            return in_string;
        }

        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }

        // Check for string delimiters
        if ch == '"' || ch == '\'' || ch == '`' {
            if !in_string {
                in_string = true;
                string_char = Some(ch);
            } else if Some(ch) == string_char {
                in_string = false;
                string_char = None;
            }
        }
    }

    in_string
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_forbidden_patterns() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        // hollowcheck:ignore-file forbidden_pattern - Test fixture
        let todo_marker = "TODO";
        std::fs::write(
            &file_path,
            format!(
                r#"
func main() {{
    // {}: implement this
    fmt.Println("Hello")
}}
"#,
                todo_marker
            ),
        )
        .unwrap();

        let patterns = vec![ForbiddenPattern {
            pattern: todo_marker.to_string(),
            description: Some("Remove TODO comments".to_string()),
        }];

        let result = detect_forbidden_patterns(&[&file_path], &patterns).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, ViolationRule::ForbiddenPattern);
        assert_eq!(result.violations[0].line, 3);
    }

    #[test]
    fn test_skip_pattern_in_string() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        let todo_marker = "TODO";
        std::fs::write(
            &file_path,
            format!(
                r#"
var msg = "{}: this is in a string"
// {}: this is a real todo
"#,
                todo_marker, todo_marker
            ),
        )
        .unwrap();

        let patterns = vec![ForbiddenPattern {
            pattern: todo_marker.to_string(),
            description: None,
        }];

        let result = detect_forbidden_patterns(&[&file_path], &patterns).unwrap();
        // Should only find the one in the comment, not the one in the string
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].line, 3);
    }

    #[test]
    fn test_is_inside_string_literal() {
        // Not in string
        assert!(!is_inside_string_literal("hello world", 0));

        // In double-quoted string
        assert!(is_inside_string_literal(r#""hello world""#, 3));

        // After string
        assert!(!is_inside_string_literal(r#""hello" world"#, 9));

        // Escaped quote
        assert!(is_inside_string_literal(r#""hello \" world""#, 10));
    }
}

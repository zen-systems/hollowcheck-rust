//! Inline suppression of violations via comments.
//!
//! Supports suppression comments like:
//! - `// hollowcheck:ignore <rule> - <reason>`
//! - `// hollowcheck:ignore-next-line <rule> - <reason>`
//! - `// hollowcheck:ignore-file <rule> - <reason>`

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::{Violation, ViolationRule};

/// How a suppression applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuppressionType {
    /// Applies to the same line
    Line,
    /// Applies to the next line
    NextLine,
    /// Applies to the entire file
    File,
}

/// An inline suppression directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suppression {
    /// Rule to suppress (e.g., "forbidden_pattern") or "*" for all
    pub rule: String,
    /// Human-readable reason
    pub reason: String,
    /// File containing the suppression
    pub file: String,
    /// Line number (0 for file-level)
    pub line: usize,
    /// How the suppression applies
    pub suppression_type: SuppressionType,
}

/// A violation that was suppressed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressedViolation {
    pub violation: Violation,
    pub suppression: Suppression,
}

lazy_static::lazy_static! {
    /// Patterns for matching suppression comments.
    static ref SUPPRESSION_PATTERNS: Vec<Regex> = vec![
        // Go/JS/TS style: // hollowcheck:...
        Regex::new(r"//\s*hollowcheck:(ignore(?:-file|-next-line)?)\s+(\S+)\s*(?:-\s*(.*))?").unwrap(),
        // Python/Shell style: # hollowcheck:...
        Regex::new(r"#\s*hollowcheck:(ignore(?:-file|-next-line)?)\s+(\S+)\s*(?:-\s*(.*))?").unwrap(),
        // Block comment style: /* hollowcheck:... */
        Regex::new(r"/\*\s*hollowcheck:(ignore(?:-file|-next-line)?)\s+(\S+)\s*(?:-\s*(.*?))?\s*\*/").unwrap(),
        // HTML comment style: <!-- hollowcheck:... -->
        Regex::new(r"<!--\s*hollowcheck:(ignore(?:-file|-next-line)?)\s+(\S+)\s*(?:-\s*(.*?))?\s*-->").unwrap(),
    ];

    /// Comment prefixes by file extension.
    static ref COMMENT_PREFIXES: HashMap<&'static str, Vec<&'static str>> = {
        let mut m = HashMap::new();
        m.insert("go", vec!["//", "/*"]);
        m.insert("js", vec!["//", "/*"]);
        m.insert("ts", vec!["//", "/*"]);
        m.insert("tsx", vec!["//", "/*"]);
        m.insert("jsx", vec!["//", "/*"]);
        m.insert("py", vec!["#"]);
        m.insert("rb", vec!["#"]);
        m.insert("sh", vec!["#"]);
        m.insert("bash", vec!["#"]);
        m.insert("yaml", vec!["#"]);
        m.insert("yml", vec!["#"]);
        m.insert("c", vec!["//", "/*"]);
        m.insert("cpp", vec!["//", "/*"]);
        m.insert("h", vec!["//", "/*"]);
        m.insert("hpp", vec!["//", "/*"]);
        m.insert("java", vec!["//", "/*"]);
        m.insert("kt", vec!["//", "/*"]);
        m.insert("rs", vec!["//", "/*"]);
        m.insert("md", vec!["<!--"]);
        m.insert("html", vec!["<!--"]);
        m.insert("xml", vec!["<!--"]);
        m
    };
}

/// Parse suppression directives from file content.
pub fn parse_suppressions(file_path: &str, content: &str) -> Vec<Suppression> {
    let mut suppressions = Vec::new();
    let mut in_package_block = true;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim();

        // Check if we've passed the header section (for file-level suppressions)
        if in_package_block && !is_comment_or_empty(trimmed, file_path) {
            in_package_block = false;
        }

        // Try each suppression pattern
        for pattern in SUPPRESSION_PATTERNS.iter() {
            if let Some(caps) = pattern.captures(line) {
                let directive = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let rule = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let reason = caps
                    .get(3)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();

                let suppression_type = match directive {
                    "ignore-file" => {
                        // File-level suppressions must be at the top of the file
                        if !in_package_block && line_number > 10 {
                            continue;
                        }
                        SuppressionType::File
                    }
                    "ignore-next-line" => SuppressionType::NextLine,
                    "ignore" => {
                        // Check if there's content before the suppression directive.
                        // If the suppression is alone on the line, treat as next-line.
                        // If there's other content (code or comments), treat as line.
                        if let Some(m) = caps.get(0) {
                            let before_match = &line[..m.start()];
                            let has_content_before = before_match.trim().trim_start_matches('/').trim_start_matches('#').trim_start_matches("/*").trim_start_matches("<!--").trim().is_empty();
                            if has_content_before {
                                SuppressionType::NextLine
                            } else {
                                SuppressionType::Line
                            }
                        } else {
                            SuppressionType::NextLine
                        }
                    }
                    _ => continue,
                };

                suppressions.push(Suppression {
                    rule: rule.to_string(),
                    reason,
                    file: file_path.to_string(),
                    line: if suppression_type == SuppressionType::File {
                        0
                    } else {
                        line_number
                    },
                    suppression_type,
                });
                break; // Only one suppression per line
            }
        }
    }

    suppressions
}

/// Check if a line is a comment or empty for the given file type.
fn is_comment_or_empty(line: &str, file_path: &str) -> bool {
    if line.is_empty() {
        return true;
    }

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let default_prefixes: &[&str] = &["//", "#", "/*", "<!--"];
    let prefixes = COMMENT_PREFIXES
        .get(ext)
        .map(|v| v.as_slice())
        .unwrap_or(default_prefixes);

    for prefix in prefixes {
        if line.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// Check if a violation matches a suppression.
pub fn matches_suppression(violation: &Violation, suppression: &Suppression) -> bool {
    // Must be same file
    if violation.file != suppression.file {
        return false;
    }

    // Must match rule (or suppression is for all rules with "*")
    if suppression.rule != "*" {
        if let Some(rule) = ViolationRule::from_str(&suppression.rule) {
            if violation.rule != rule {
                return false;
            }
        } else if suppression.rule != violation.rule.as_str() {
            return false;
        }
    }

    match suppression.suppression_type {
        SuppressionType::File => true,
        SuppressionType::Line => violation.line == suppression.line,
        SuppressionType::NextLine => violation.line == suppression.line + 1,
    }
}

/// Separate violations into active and suppressed based on suppressions.
pub fn filter_suppressed(
    violations: Vec<Violation>,
    suppressions: &[Suppression],
) -> (Vec<Violation>, Vec<SuppressedViolation>) {
    let mut active = Vec::new();
    let mut suppressed = Vec::new();

    for violation in violations {
        let mut was_suppressed = false;
        for suppression in suppressions {
            if matches_suppression(&violation, suppression) {
                suppressed.push(SuppressedViolation {
                    violation: violation.clone(),
                    suppression: suppression.clone(),
                });
                was_suppressed = true;
                break;
            }
        }
        if !was_suppressed {
            active.push(violation);
        }
    }

    (active, suppressed)
}

/// Collect suppressions from all files.
pub fn collect_suppressions<P: AsRef<Path>>(
    files: &[P],
) -> anyhow::Result<HashMap<String, Vec<Suppression>>> {
    let mut result = HashMap::new();

    for file in files {
        let path = file.as_ref();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // Skip files we can't read
        };

        let file_str = path.to_string_lossy().to_string();
        let suppressions = parse_suppressions(&file_str, &content);
        if !suppressions.is_empty() {
            result.insert(file_str, suppressions);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Severity;

    #[test]
    fn test_parse_suppressions_go_style() {
        let content = r#"
package main

// hollowcheck:ignore-file forbidden_pattern - Generated code
func main() {
    // TODO: implement // hollowcheck:ignore forbidden_pattern - Expected
}
"#;
        let suppressions = parse_suppressions("main.go", content);
        assert_eq!(suppressions.len(), 2);

        // File-level suppression
        assert_eq!(suppressions[0].suppression_type, SuppressionType::File);
        assert_eq!(suppressions[0].rule, "forbidden_pattern");

        // Line-level suppression
        assert_eq!(suppressions[1].suppression_type, SuppressionType::Line);
    }

    #[test]
    fn test_parse_suppressions_next_line() {
        let content = r#"
// hollowcheck:ignore-next-line mock_data - Test data
var testUrl = "example.com"
"#;
        let suppressions = parse_suppressions("test.go", content);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].suppression_type, SuppressionType::NextLine);
        assert_eq!(suppressions[0].line, 2);
    }

    #[test]
    fn test_matches_suppression() {
        let violation = Violation {
            rule: ViolationRule::ForbiddenPattern,
            message: "TODO found".to_string(),
            file: "main.go".to_string(),
            line: 5,
            severity: Severity::Error,
        };

        // File-level suppression
        let file_suppression = Suppression {
            rule: "forbidden_pattern".to_string(),
            reason: "Generated".to_string(),
            file: "main.go".to_string(),
            line: 0,
            suppression_type: SuppressionType::File,
        };
        assert!(matches_suppression(&violation, &file_suppression));

        // Next-line suppression
        let next_line_suppression = Suppression {
            rule: "forbidden_pattern".to_string(),
            reason: "Expected".to_string(),
            file: "main.go".to_string(),
            line: 4,
            suppression_type: SuppressionType::NextLine,
        };
        assert!(matches_suppression(&violation, &next_line_suppression));

        // Wrong rule
        let wrong_rule = Suppression {
            rule: "mock_data".to_string(),
            reason: "".to_string(),
            file: "main.go".to_string(),
            line: 0,
            suppression_type: SuppressionType::File,
        };
        assert!(!matches_suppression(&violation, &wrong_rule));

        // Wildcard rule
        let wildcard = Suppression {
            rule: "*".to_string(),
            reason: "".to_string(),
            file: "main.go".to_string(),
            line: 0,
            suppression_type: SuppressionType::File,
        };
        assert!(matches_suppression(&violation, &wildcard));
    }
}

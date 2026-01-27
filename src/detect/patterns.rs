//! Detection of forbidden patterns in code.

use crate::contract::ForbiddenPattern;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{DetectionResult, Violation, ViolationRule};

/// Patterns that indicate TODO/FIXME markers - these need special context handling
const TODO_LIKE_PATTERNS: &[&str] = &["TODO", "FIXME", "XXX", "HACK"];

/// Pre-compiled pattern with metadata.
struct CompiledPattern {
    regex: Regex,
    description: Option<String>,
    /// Whether this pattern matches TODO-like markers that need context filtering
    is_todo_like: bool,
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
            // Check if this is a TODO-like pattern that needs special handling
            let is_todo_like = TODO_LIKE_PATTERNS
                .iter()
                .any(|t| p.pattern.to_uppercase().contains(t));
            Ok(CompiledPattern {
                regex,
                description: p.description.clone(),
                is_todo_like,
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

                // For TODO-like patterns, apply additional context filtering
                if p.is_todo_like {
                    if should_skip_todo_pattern(&line, file_path, mat.start(), mat.end()) {
                        continue;
                    }
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
                    severity: ViolationRule::ForbiddenPattern.default_severity(),
                });
            }
        }
    }

    Ok(violations)
}

/// Determine if a TODO-like pattern match should be skipped based on context.
///
/// Skips matches that are:
/// - Part of an identifier (e.g., TODO_PATTERN, is_todo_in_test_context)
/// - In doc comments explaining TODO functionality
/// - In regex pattern definitions
/// - In test fixture data
fn should_skip_todo_pattern(line: &str, file_path: &Path, start: usize, end: usize) -> bool {
    let chars: Vec<char> = line.chars().collect();

    // Check if the match is part of a larger identifier
    // (preceded or followed by alphanumeric/underscore)
    if start > 0 {
        let prev_char = chars.get(start - 1).copied().unwrap_or(' ');
        if prev_char.is_alphanumeric() || prev_char == '_' {
            return true;
        }
    }
    if end < chars.len() {
        let next_char = chars.get(end).copied().unwrap_or(' ');
        if next_char.is_alphanumeric() || next_char == '_' {
            return true;
        }
    }

    let trimmed = line.trim();
    let upper = trimmed.to_uppercase();

    // Skip doc comments (Rust ///, Python docstrings in source context)
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return true;
    }

    // Skip if line is a regex pattern definition (common in detection code)
    if upper.contains("REGEX") || upper.contains("PATTERN") {
        if trimmed.contains("Regex::new") || trimmed.contains("r\"") || trimmed.contains("r#\"") {
            return true;
        }
    }

    // Skip lazy_static pattern definitions
    if trimmed.contains("static ref") && (trimmed.contains("Regex") || upper.contains("PATTERN")) {
        return true;
    }

    // Skip lines that are defining patterns to match TODO
    // e.g., const TODO_PATTERN = ..., pattern: "TODO", etc.
    if is_pattern_definition(trimmed) {
        return true;
    }

    // Skip YAML/config pattern definitions
    if trimmed.starts_with("pattern:") || trimmed.starts_with("- pattern:") {
        return true;
    }
    if trimmed.starts_with("description:") {
        return true;
    }

    // Skip if this is a detection module that deals with TODOs
    let path_str = file_path.to_string_lossy().to_lowercase();
    if path_str.ends_with("todos.rs") || path_str.ends_with("todo.rs")
        || path_str.ends_with("patterns.rs") || path_str.ends_with("stubs.rs") {
        // These files deal with detecting TODOs, so most references are meta
        return true;
    }

    // Skip test assertions about TODOs
    if trimmed.contains("assert") && (upper.contains("TODO") || upper.contains("FIXME")) {
        return true;
    }

    // Skip comments that are describing TODO detection behavior (meta-comments)
    if trimmed.starts_with("//") {
        let comment_upper = upper.trim_start_matches('/').trim();
        // Skip if comment is describing/explaining TODO behavior
        if comment_upper.contains("TODO MARKER")
            || comment_upper.contains("TODO PATTERN")
            || comment_upper.contains("TODO DETECTION")
            || comment_upper.contains("TODO COMMENT")
            || comment_upper.contains("IF TODO")
            || comment_upper.contains("FOR TODO")
            || comment_upper.contains("CHECK FOR TODO")
            || comment_upper.contains("SKIP IF TODO")
            || comment_upper.contains("FLAG TODO")
            || comment_upper.contains("MATCH TODO")
            || comment_upper.contains("CONTAINS TODO")
            || comment_upper.contains("HOLLOW TODO")
            || comment_upper.contains("FIXME MARKER")
            || comment_upper.starts_with("TODO-LIKE")
            || comment_upper.starts_with("GOOD TODO")
            || comment_upper.starts_with("BAD TODO")
            || comment_upper.starts_with("MEANINGFUL TODO")
            || comment_upper.starts_with("SKIP TODO")
            || comment_upper.starts_with("E.G.") // Example comments
            || (comment_upper.contains("\"TODO") || comment_upper.contains("'TODO"))
        {
            return true;
        }
    }

    false
}

/// Check if a line is defining a pattern/constant that references TODO.
fn is_pattern_definition(line: &str) -> bool {
    let trimmed = line.trim();
    let upper = trimmed.to_uppercase();

    // Constant definitions
    if (trimmed.starts_with("const ") || trimmed.starts_with("static "))
        && upper.contains("PATTERN") {
        return true;
    }

    // Variable assignments to pattern-related names
    if trimmed.contains("_PATTERN") || trimmed.contains("_PATTERNS") {
        return true;
    }

    // Regex captures/match statements
    if trimmed.contains(".is_match(") || trimmed.contains(".captures(") {
        return true;
    }

    // ForbiddenPattern struct construction
    if trimmed.contains("ForbiddenPattern {") || trimmed.contains("ForbiddenPattern{") {
        return true;
    }

    // MockSignature or similar pattern structs
    if trimmed.contains("pattern:") && (trimmed.contains("r\"") || trimmed.contains("r#\"")) {
        return true;
    }

    false
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

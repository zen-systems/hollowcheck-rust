//! Context-aware TODO detection.
//!
//! Distinguishes between meaningful TODOs (with specific context) and
//! hollow TODOs (generic placeholders without useful information).
//!
//! # Good TODOs (not flagged):
//! - `// TODO: Use io_uring for zero-copy when kernel >= 5.19`
//! - `// TODO(jsmith): Optimize query performance per #1234`
//! - `// FIXME: Race condition when concurrent writes exceed buffer`
//!
//! # Hollow TODOs (flagged as warnings):
//! - `// TODO: Implement this function`
//! - `// TODO`
//! - `// FIXME: fix this`
//! - `// TODO: finish later`

use lazy_static::lazy_static;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

lazy_static! {
    /// Pattern to match TODO/FIXME markers
    static ref TODO_PATTERN: Regex = Regex::new(
        r"(?i)\b(TODO|FIXME|XXX|HACK)\b\s*:?\s*(.*)"
    ).unwrap();

    /// Patterns indicating hollow/generic TODO content.
    /// These patterns match generic placeholder text without technical specifics.
    static ref HOLLOW_PATTERNS: Vec<Regex> = vec![
        // Empty or near-empty
        Regex::new(r"(?i)^$").unwrap(),
        Regex::new(r"(?i)^\s*$").unwrap(),

        // Generic placeholder phrases - match verb + generic object, with optional filler words
        // "implement this", "implement this function", "implement here", etc.
        Regex::new(r"(?i)^\s*implement\s+(this|here|later|the)(\s+(function|method|code|logic|feature))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*implement\s*$").unwrap(),

        Regex::new(r"(?i)^\s*finish\s+(this|here|later|the)(\s+(function|method|code|logic|feature|implementation))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*finish\s+(implementation|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*finish\s*$").unwrap(),

        Regex::new(r"(?i)^\s*complete\s+(this|here|later|the)(\s+(function|method|code|logic|feature|implementation))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*complete\s+(implementation|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*complete\s*$").unwrap(),

        Regex::new(r"(?i)^\s*add\s+(this|here|later|the)(\s+(function|method|code|logic|feature|implementation))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*add\s+(code|implementation|logic)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*add\s*$").unwrap(),

        Regex::new(r"(?i)^\s*fix\s+(this|here|it|later|the)(\s+(bug|issue|error|problem))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*fix\s+(bug|issue|error|problem|it|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*fix\s*$").unwrap(),

        Regex::new(r"(?i)^\s*do\s+(this|something|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*do\s*$").unwrap(),

        Regex::new(r"(?i)^\s*handle\s+(this|here|it|the)(\s+(error|case|exception))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*handle\s+(error|case|exception)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*handle\s*$").unwrap(),

        Regex::new(r"(?i)^\s*write\s+(this|here|the)(\s+(code|implementation|function|method))?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*write\s+(code|implementation)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*write\s*$").unwrap(),

        Regex::new(r"(?i)^\s*fill\s*(this\s*)?in\s*(later|here)?\s*$").unwrap(),

        // Single-word hollow markers
        Regex::new(r"(?i)^\s*placeholder\s*$").unwrap(),
        Regex::new(r"(?i)^\s*stub\s*$").unwrap(),
        Regex::new(r"(?i)^\s*tbd\s*$").unwrap(),
        Regex::new(r"(?i)^\s*wip\s*$").unwrap(),

        // "not implemented" variants
        Regex::new(r"(?i)^\s*not\s+implemented\s*(yet)?\s*$").unwrap(),
        Regex::new(r"(?i)^\s*needs?\s+(implementation|work|to\s+be\s+done)\s*$").unwrap(),

        // Other generic verbs
        Regex::new(r"(?i)^\s*change\s+(this|me|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*update\s+(this|here|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*refactor\s+(this|here|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*cleanup\s+(this|here|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*clean\s+up\s+(this|here|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*remove\s+(this|me|later)\s*$").unwrap(),
        Regex::new(r"(?i)^\s*delete\s+(this|me|later)\s*$").unwrap(),
    ];
}

/// Detect hollow TODOs in the given files.
///
/// Returns violations for TODOs that lack meaningful context.
pub fn detect_hollow_todos<P: AsRef<Path>>(files: &[P]) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    for file in files {
        let violations = scan_file_for_hollow_todos(file.as_ref())?;
        result.violations.extend(violations);
        result.scanned += 1;
    }

    Ok(result)
}

/// Check if a TODO has meaningful context.
///
/// Returns true if the TODO is hollow (should be flagged).
fn is_hollow_todo(content: &str) -> bool {
    let trimmed = content.trim();

    // Empty content is definitely hollow
    if trimmed.is_empty() {
        return true;
    }

    // Check against hollow patterns
    for pattern in HOLLOW_PATTERNS.iter() {
        if pattern.is_match(trimmed) {
            return true;
        }
    }

    false
}

/// Scan a single file for hollow TODOs.
fn scan_file_for_hollow_todos(file_path: &Path) -> anyhow::Result<Vec<Violation>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut violations = Vec::new();
    let file_str = file_path.to_string_lossy().to_string();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let line_number = line_num + 1;

        // Skip if line is inside a string literal (simplified check)
        if is_likely_string_content(&line) {
            continue;
        }

        // Check for TODO markers
        if let Some(caps) = TODO_PATTERN.captures(&line) {
            let marker = caps.get(1).map(|m| m.as_str()).unwrap_or("TODO");
            let content = caps.get(2).map(|m| m.as_str()).unwrap_or("");

            // Check if this is a hollow TODO
            if is_hollow_todo(content) {
                let msg = if content.trim().is_empty() {
                    format!("{} marker without context", marker.to_uppercase())
                } else {
                    format!(
                        "{} with hollow context: {:?}",
                        marker.to_uppercase(),
                        content.trim()
                    )
                };

                violations.push(Violation {
                    rule: ViolationRule::HollowTodo,
                    message: msg,
                    file: file_str.clone(),
                    line: line_number,
                    severity: Severity::Warning,
                });
            }
        }
    }

    Ok(violations)
}

/// Simple check if a line is likely a string literal (not a comment).
///
/// This is a heuristic to avoid flagging TODOs that are in user-facing strings.
fn is_likely_string_content(line: &str) -> bool {
    let trimmed = line.trim();

    // If line starts with a quote and contains TODO, it's likely a string
    if (trimmed.starts_with('"') || trimmed.starts_with('\''))
        && trimmed.to_uppercase().contains("TODO")
    {
        return true;
    }

    // Check for common patterns of string assignments with TODO content
    // e.g., let msg = "TODO: implement this"
    let string_assignment =
        regex::Regex::new(r#"(?i)=\s*["'].*(?:TODO|FIXME|XXX|HACK).*["']"#).unwrap();
    if string_assignment.is_match(trimmed) {
        return true;
    }

    // Check for function calls with TODO in string args
    // e.g., println!("TODO: implement this")
    let fn_call_string =
        regex::Regex::new(r#"(?i)\(\s*["'].*(?:TODO|FIXME|XXX|HACK).*["']\s*\)"#).unwrap();
    if fn_call_string.is_match(trimmed) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_hollow_todo() {
        // Hollow TODOs - generic placeholders without specifics
        assert!(is_hollow_todo(""));
        assert!(is_hollow_todo("implement"));
        assert!(is_hollow_todo("implement this"));
        assert!(is_hollow_todo("Implement this function"));
        assert!(is_hollow_todo("finish later"));
        assert!(is_hollow_todo("fix this"));
        assert!(is_hollow_todo("fix"));
        assert!(is_hollow_todo("TBD"));
        assert!(is_hollow_todo("WIP"));
        assert!(is_hollow_todo("stub"));
        assert!(is_hollow_todo("placeholder"));
        assert!(is_hollow_todo("not implemented"));
        assert!(is_hollow_todo("needs implementation"));
        assert!(is_hollow_todo("add code"));
        assert!(is_hollow_todo("add this"));

        // Meaningful TODOs (not hollow) - contain technical specifics
        assert!(!is_hollow_todo("Use io_uring for zero-copy when kernel >= 5.19"));
        assert!(!is_hollow_todo("Optimize query performance per #1234"));
        assert!(!is_hollow_todo("Race condition when concurrent writes exceed buffer"));
        assert!(!is_hollow_todo("See https://example.com/issue/123"));
        assert!(!is_hollow_todo("@jsmith needs to review this"));
        assert!(!is_hollow_todo("Upgrade to v2.0 API"));
        assert!(!is_hollow_todo("Add retry logic with exponential backoff"));
        assert!(!is_hollow_todo("Implement caching for database queries"));
        assert!(!is_hollow_todo("Fix memory leak in worker thread pool"));
    }

    #[test]
    fn test_detect_hollow_todos() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");
        std::fs::write(
            &file_path,
            r#"
fn main() {
    // TODO: implement this function
    let x = 1;

    // TODO: Use io_uring for zero-copy when kernel >= 5.19
    do_something();

    // FIXME
    broken_code();

    // TODO: Optimize with batching per #456
    process();
}
"#,
        )
        .unwrap();

        let result = detect_hollow_todos(&[&file_path]).unwrap();

        // Should flag "implement this function" and empty FIXME
        // Should NOT flag the io_uring TODO or the #456 reference
        assert_eq!(result.violations.len(), 2);
        assert!(result.violations.iter().any(|v| v.line == 3)); // implement this function
        assert!(result.violations.iter().any(|v| v.line == 9)); // empty FIXME
    }

    #[test]
    fn test_skip_string_content() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");
        std::fs::write(
            &file_path,
            r#"
fn main() {
    let msg = "TODO: implement this";
    println!("FIXME: fix this");
}
"#,
        )
        .unwrap();

        let result = detect_hollow_todos(&[&file_path]).unwrap();

        // Should not flag TODOs in strings
        assert_eq!(result.violations.len(), 0);
    }
}

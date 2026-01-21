//! AST-backed stub/hollow function detection.
//!
//! This module uses tree-sitter AST analysis to detect hollow function
//! implementations, replacing regex-based heuristics with precise AST inspection.

use std::path::Path;

use crate::analysis::{
    get_analyzer, HollowBodyKind, StubDetector, StubDetectorConfig, StubFinding,
};

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Configuration for stub detection in contracts.
#[derive(Debug, Clone, Default)]
pub struct StubDetectionConfig {
    /// Enable detection of empty function bodies.
    pub detect_empty: bool,
    /// Enable detection of panic-only bodies.
    pub detect_panic: bool,
    /// Enable detection of nil-return-only bodies.
    pub detect_nil_return: bool,
    /// Enable detection of TODO-comment-only bodies.
    pub detect_todo_comment: bool,
    /// Function names to skip.
    pub skip_functions: Vec<String>,
}

impl StubDetectionConfig {
    /// Create a default configuration (all detections enabled except nil-return).
    pub fn default_enabled() -> Self {
        Self {
            detect_empty: true,
            detect_panic: true,
            detect_nil_return: false,
            detect_todo_comment: true,
            skip_functions: vec!["main".to_string(), "init".to_string()],
        }
    }
}

/// Detect stub/hollow functions in files using AST analysis.
///
/// This function uses tree-sitter to parse files and inspect function bodies
/// for stub patterns like empty bodies, panic-only bodies, etc.
pub fn detect_stub_functions<P: AsRef<Path>>(
    files: &[P],
    config: Option<&StubDetectionConfig>,
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    // Build detector config
    let detector_config = if let Some(cfg) = config {
        StubDetectorConfig {
            detect_empty: cfg.detect_empty,
            detect_panic: cfg.detect_panic,
            detect_nil_return: cfg.detect_nil_return,
            detect_todo_comment: cfg.detect_todo_comment,
            min_complexity: 0,
            skip_functions: cfg.skip_functions.clone(),
            skip_receivers: vec![],
        }
    } else {
        StubDetectorConfig::default()
    };

    let detector = StubDetector::with_config(detector_config);

    for file in files {
        let path = file.as_ref();

        // Get file extension
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Get analyzer for this extension
        let analyzer = match get_analyzer(ext) {
            Some(a) => a,
            None => continue, // Skip unsupported files
        };

        // Read and parse file
        let source = match std::fs::read(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let parsed = match analyzer.parse(path, &source) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let facts = match analyzer.extract_facts(&parsed) {
            Ok(f) => f,
            Err(_) => continue,
        };

        result.scanned += 1;

        // Detect stubs
        let findings = detector.detect(&facts);

        // Convert findings to violations
        for finding in findings {
            let violation = stub_finding_to_violation(finding, path);
            result.add_violation(violation);
        }
    }

    Ok(result)
}

/// Convert a StubFinding to a Violation.
fn stub_finding_to_violation(finding: StubFinding, file_path: &Path) -> Violation {
    let severity = match finding.kind {
        HollowBodyKind::Empty => Severity::Error,
        HollowBodyKind::PanicOnly => Severity::Error,
        HollowBodyKind::TodoCommentOnly => Severity::Warning,
        HollowBodyKind::NilReturnOnly => Severity::Warning,
    };

    let message = format!(
        "stub function {:?}: {}",
        finding.qualified_name,
        finding.kind.description()
    );

    Violation {
        rule: ViolationRule::StubFunction,
        message,
        file: file_path.to_string_lossy().to_string(),
        line: finding.span.start_line,
        severity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_analyzers() {
        crate::analysis::register_analyzers();
    }

    #[test]
    fn test_detect_empty_go_function() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

func empty() {
}

func notEmpty() {
    println("hello")
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("empty"));
        assert!(result.violations[0].message.contains("empty function body"));
    }

    #[test]
    fn test_detect_panic_go_function() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

func notImplemented() {
    panic("not implemented")
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("panic"));
    }

    #[test]
    fn test_detect_todo_comment_go_function() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

func placeholder() {
    // TODO: implement this
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("TODO"));
    }

    #[test]
    fn test_skip_main_function() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

func main() {
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 0);
    }

    #[test]
    fn test_detect_rust_unimplemented() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");
        fs::write(
            &file_path,
            r#"
fn placeholder() {
    unimplemented!()
}

fn todo_func() {
    todo!()
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 2);
    }

    #[test]
    fn test_no_stub_real_implementation() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

func realFunc(x int) int {
    if x > 0 {
        return x * 2
    }
    return 0
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        assert_eq!(result.violations.len(), 0);
    }
}

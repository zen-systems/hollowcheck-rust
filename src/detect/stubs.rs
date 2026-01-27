//! AST-backed stub/hollow function detection.
//!
//! This module uses tree-sitter AST analysis to detect hollow function
//! implementations, replacing regex-based heuristics with precise AST inspection.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::analysis::{
    get_analyzer, HollowBodyKind, StubDetector, StubDetectorConfig, StubFinding,
};

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Check if stub detection should be skipped for a file/function.
///
/// Skips test files and intentional test doubles to avoid false positives
/// in test code where stubs are intentional.
fn should_skip_stub_detection(file_path: &Path, function_name: &str) -> bool {
    let path_str = file_path.to_string_lossy().to_lowercase();
    let func_lower = function_name.to_lowercase();

    // Skip test directories
    if path_str.contains("/testing/")
        || path_str.contains("/testdata/")
        || path_str.contains("/test/")
        || path_str.contains("/tests/")
        || path_str.contains("/__tests__/")
    {
        return true;
    }

    // Skip test files by extension pattern
    if path_str.ends_with("_test.go")
        || path_str.ends_with("_test.py")
        || path_str.ends_with(".test.js")
        || path_str.ends_with(".test.ts")
        || path_str.ends_with(".spec.js")
        || path_str.ends_with(".spec.ts")
    {
        return true;
    }

    // Skip fake/mock files
    if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
        let file_lower = file_name.to_lowercase();
        if file_lower.starts_with("fake_")
            || file_lower.starts_with("mock_")
            || file_lower.contains("fake")
            || file_lower.contains("mock")
        {
            return true;
        }
    }

    // Skip test doubles and generated code by function name
    if func_lower.starts_with("fake")
        || func_lower.starts_with("mock")
        || func_lower.starts_with("stub")
        || func_lower.starts_with("noop")
        || func_lower.starts_with("dummy")
        || func_lower.starts_with("test")
        || func_lower.contains("unimplemented")
    {
        return true;
    }

    false
}

/// Check if a function is a legitimate no-op for interface compliance.
///
/// Some frameworks (especially Kubernetes) have interfaces where empty
/// implementations are intentional and correct - they represent optional
/// hooks or default behaviors that don't need to do anything.
fn is_legitimate_noop(function_name: &str, file_path: &Path, body_kind: &HollowBodyKind) -> bool {
    let func_lower = function_name.to_lowercase();
    let path_str = file_path.to_string_lossy().to_lowercase();

    // Only apply to empty bodies, not panic/todo
    if !matches!(body_kind, HollowBodyKind::Empty) {
        return false;
    }

    // Kubernetes API machinery patterns (interface compliance)
    let kubernetes_noop_methods = [
        "canonicalize",           // Strategy pattern - optional normalization
        "destroy",                // Resource cleanup - may be empty
        "preparefor",             // Preparation hooks - may be empty
        "enablemetrics",          // Metrics - may be disabled
        "setallocated",           // Metrics recording - may be no-op
        "setavailable",
        "incrementallocations",
        "incrementallocationerrors",
    ];

    for pattern in &kubernetes_noop_methods {
        if func_lower.contains(pattern) {
            return true;
        }
    }

    // Registry/storage patterns in Kubernetes
    if path_str.contains("/registry/") && (
        func_lower.ends_with("rest.destroy") ||
        func_lower.contains("strategy.") ||
        func_lower.contains("preparefor")
    ) {
        return true;
    }

    false
}

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
///
/// Files are processed in parallel using rayon for better performance on
/// large codebases.
pub fn detect_stub_functions<P: AsRef<Path> + Sync>(
    files: &[P],
    config: Option<&StubDetectionConfig>,
) -> anyhow::Result<DetectionResult> {
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
    let scanned = AtomicUsize::new(0);

    // Process files in parallel
    let file_results: Vec<Vec<Violation>> = files
        .par_iter()
        .filter_map(|file| {
            let path = file.as_ref();

            // Get file extension
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // Get analyzer for this extension
            let analyzer = get_analyzer(ext)?;

            // Read and parse file
            let source = std::fs::read(path).ok()?;
            let parsed = analyzer.parse(path, &source).ok()?;
            let facts = analyzer.extract_facts(&parsed).ok()?;

            scanned.fetch_add(1, Ordering::Relaxed);

            // Detect stubs
            let findings = detector.detect(&facts);

            // Convert findings to violations, filtering out test code and legitimate no-ops
            let violations: Vec<Violation> = findings
                .into_iter()
                .filter(|finding| {
                    // Extract the simple function name from qualified name
                    let func_name = finding
                        .qualified_name
                        .split("::")
                        .last()
                        .or_else(|| finding.qualified_name.split('.')
                            .last())
                        .unwrap_or(&finding.qualified_name);

                    // Skip test code
                    if should_skip_stub_detection(path, func_name) {
                        return false;
                    }

                    // Skip legitimate interface compliance no-ops
                    if is_legitimate_noop(func_name, path, &finding.kind) {
                        return false;
                    }

                    true
                })
                .map(|finding| stub_finding_to_violation(finding, path))
                .collect();

            Some(violations)
        })
        .collect();

    // Merge results
    let mut result = DetectionResult::new();
    result.scanned = scanned.load(Ordering::Relaxed);
    for violations in file_results {
        for v in violations {
            result.add_violation(v);
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

    #[test]
    fn test_skip_kubernetes_noop_methods() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("strategy.go");
        fs::write(
            &file_path,
            r#"
package core

// Canonicalize is intentionally empty for this strategy
func (s *MyType) Canonicalize(obj runtime.Object) {
}

// PrepareForCreate is intentionally empty
func (s *MyType) PrepareForCreate(ctx context.Context, obj runtime.Object) {
}

// Destroy is intentionally empty - no cleanup needed
func (r *REST) Destroy() {
}

// This one should still be detected (not a known pattern)
func (s *MyType) DoSomething() {
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        // Only DoSomething should be flagged, not the known no-op patterns
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("DoSomething"));
    }

    #[test]
    fn test_noop_only_applies_to_empty_bodies() {
        init_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.go");
        fs::write(
            &file_path,
            r#"
package main

// Panic in a noop method should still be flagged
func Canonicalize() {
    panic("not implemented")
}
"#,
        )
        .unwrap();

        let config = StubDetectionConfig::default_enabled();
        let result = detect_stub_functions(&[&file_path], Some(&config)).unwrap();

        // Should be flagged because panic bodies are not legitimate no-ops
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("panic"));
    }
}

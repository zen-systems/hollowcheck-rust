//! Stub/hollow function detection using AST analysis.
//!
//! Detects functions that appear to be stubs or placeholder implementations:
//! - Empty function bodies
//! - Bodies containing only panic/unimplemented/todo! calls
//! - Bodies returning only nil/None/null
//! - Bodies containing only TODO comments

use crate::analysis::{FileFacts, FunctionBody, Span};

/// Kind of hollow/stub body detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HollowBodyKind {
    /// Empty function body (no statements).
    Empty,
    /// Body only contains panic/unimplemented/todo! call.
    PanicOnly,
    /// Body only returns nil/None/null.
    NilReturnOnly,
    /// Body only contains TODO/FIXME comment.
    TodoCommentOnly,
}

impl HollowBodyKind {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            HollowBodyKind::Empty => "empty function body",
            HollowBodyKind::PanicOnly => "only contains panic/unimplemented/todo! call",
            HollowBodyKind::NilReturnOnly => "only returns nil/None",
            HollowBodyKind::TodoCommentOnly => "only contains TODO comment",
        }
    }

    /// Get severity level (0 = most severe).
    pub fn severity_level(&self) -> u8 {
        match self {
            HollowBodyKind::Empty => 0,
            HollowBodyKind::PanicOnly => 1,
            HollowBodyKind::TodoCommentOnly => 2,
            HollowBodyKind::NilReturnOnly => 3,
        }
    }
}

impl std::fmt::Display for HollowBodyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// A finding from stub detection.
#[derive(Debug, Clone)]
pub struct StubFinding {
    /// The function/method name.
    pub name: String,
    /// Qualified name (receiver.name for methods).
    pub qualified_name: String,
    /// The file path.
    pub file: String,
    /// The span of the function declaration.
    pub span: Span,
    /// The kind of stub detected.
    pub kind: HollowBodyKind,
    /// The function body text (for context).
    pub body_text: String,
}

/// Configuration for stub detection.
#[derive(Debug, Clone)]
pub struct StubDetectorConfig {
    /// Report empty function bodies.
    pub detect_empty: bool,
    /// Report panic-only bodies.
    pub detect_panic: bool,
    /// Report nil-return-only bodies.
    pub detect_nil_return: bool,
    /// Report TODO-comment-only bodies.
    pub detect_todo_comment: bool,
    /// Minimum complexity threshold - functions below this are flagged.
    /// Set to 0 to disable complexity-based detection.
    pub min_complexity: i32,
    /// Skip functions with these exact names.
    pub skip_functions: Vec<String>,
    /// Skip methods on these receiver types.
    pub skip_receivers: Vec<String>,
}

impl Default for StubDetectorConfig {
    fn default() -> Self {
        Self {
            detect_empty: true,
            detect_panic: true,
            detect_nil_return: false, // Disabled by default - many legitimate returns nil
            detect_todo_comment: true,
            min_complexity: 0,
            skip_functions: vec![
                "main".to_string(),
                "init".to_string(),
            ],
            skip_receivers: vec![],
        }
    }
}

/// Stub detector that analyzes function bodies for hollow implementations.
pub struct StubDetector {
    config: StubDetectorConfig,
}

impl StubDetector {
    /// Create a new stub detector with default configuration.
    pub fn new() -> Self {
        Self {
            config: StubDetectorConfig::default(),
        }
    }

    /// Create a stub detector with custom configuration.
    pub fn with_config(config: StubDetectorConfig) -> Self {
        Self { config }
    }

    /// Detect stubs in a single file's facts.
    pub fn detect(&self, facts: &FileFacts) -> Vec<StubFinding> {
        let mut findings = Vec::new();

        for decl in &facts.declarations {
            if !decl.kind.is_callable() {
                continue;
            }

            // Check skip lists
            if self.config.skip_functions.contains(&decl.name) {
                continue;
            }

            if let Some(ref recv) = decl.receiver {
                if self.config.skip_receivers.contains(recv) {
                    continue;
                }
            }

            if let Some(ref body) = decl.body {
                if let Some(kind) = self.classify_body(body) {
                    findings.push(StubFinding {
                        name: decl.name.clone(),
                        qualified_name: decl.qualified_name(),
                        file: facts.path.clone(),
                        span: decl.span.clone(),
                        kind,
                        body_text: body.text.clone(),
                    });
                }
            }
        }

        // Sort findings by position for deterministic output
        findings.sort_by_key(|f| (f.span.start_byte, f.name.clone()));

        findings
    }

    /// Detect stubs across multiple files.
    pub fn detect_all(&self, facts: &[FileFacts]) -> Vec<StubFinding> {
        let mut all_findings = Vec::new();

        for file_facts in facts {
            all_findings.extend(self.detect(file_facts));
        }

        // Sort by file path, then position
        all_findings.sort_by(|a, b| {
            (&a.file, a.span.start_byte, &a.name)
                .cmp(&(&b.file, b.span.start_byte, &b.name))
        });

        all_findings
    }

    /// Classify a function body as a stub type, if applicable.
    fn classify_body(&self, body: &FunctionBody) -> Option<HollowBodyKind> {
        // Check TODO comment first - a body with only TODO comment has is_empty=true
        // but should be reported as TodoCommentOnly, not Empty
        if self.config.detect_todo_comment && body.has_only_todo_comment {
            return Some(HollowBodyKind::TodoCommentOnly);
        }

        // Check in order of severity
        if self.config.detect_empty && body.is_empty {
            return Some(HollowBodyKind::Empty);
        }

        if self.config.detect_panic && body.is_panic_only {
            return Some(HollowBodyKind::PanicOnly);
        }

        if self.config.detect_nil_return && body.is_nil_return_only {
            return Some(HollowBodyKind::NilReturnOnly);
        }

        None
    }
}

impl Default for StubDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{ControlFlowInfo, Declaration, DeclarationKind};

    fn make_facts(decls: Vec<Declaration>) -> FileFacts {
        FileFacts {
            path: "test.go".to_string(),
            language: "go".to_string(),
            package: Some("main".to_string()),
            declarations: decls,
            imports: vec![],
            has_parse_errors: false,
            parse_error: None,
        }
    }

    fn make_decl(name: &str, body: FunctionBody) -> Declaration {
        Declaration {
            name: name.to_string(),
            kind: DeclarationKind::Function,
            span: Span {
                start_byte: 0,
                end_byte: 100,
                start_line: 1,
                start_col: 1,
                end_line: 5,
                end_col: 1,
            },
            receiver: None,
            body: Some(body),
        }
    }

    fn make_body(
        is_empty: bool,
        is_panic_only: bool,
        is_nil_return_only: bool,
        has_only_todo_comment: bool,
    ) -> FunctionBody {
        FunctionBody {
            span: Span {
                start_byte: 10,
                end_byte: 50,
                start_line: 2,
                start_col: 1,
                end_line: 4,
                end_col: 1,
            },
            statement_count: if is_empty { 0 } else { 1 },
            is_empty,
            is_panic_only,
            is_nil_return_only,
            has_only_todo_comment,
            text: "{}".to_string(),
            control_flow: ControlFlowInfo::default(),
        }
    }

    #[test]
    fn test_detect_empty_body() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "empty",
            make_body(true, false, false, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, HollowBodyKind::Empty);
    }

    #[test]
    fn test_detect_panic_body() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "panics",
            make_body(false, true, false, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, HollowBodyKind::PanicOnly);
    }

    #[test]
    fn test_detect_todo_comment() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "placeholder",
            make_body(false, false, false, true),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, HollowBodyKind::TodoCommentOnly);
    }

    #[test]
    fn test_skip_main_function() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "main",
            make_body(true, false, false, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_nil_return_disabled_by_default() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "returnsNil",
            make_body(false, false, true, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 0); // Disabled by default
    }

    #[test]
    fn test_nil_return_enabled() {
        let config = StubDetectorConfig {
            detect_nil_return: true,
            ..Default::default()
        };
        let detector = StubDetector::with_config(config);
        let facts = make_facts(vec![make_decl(
            "returnsNil",
            make_body(false, false, true, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, HollowBodyKind::NilReturnOnly);
    }

    #[test]
    fn test_no_findings_for_real_implementation() {
        let detector = StubDetector::new();
        let facts = make_facts(vec![make_decl(
            "realFunc",
            make_body(false, false, false, false),
        )]);

        let findings = detector.detect(&facts);
        assert_eq!(findings.len(), 0);
    }
}

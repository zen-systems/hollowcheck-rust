//! Fact structures extracted from AST analysis.

use std::fmt;

/// Source location span with byte offsets and line/column positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Start byte offset (0-indexed).
    pub start_byte: usize,
    /// End byte offset (0-indexed, exclusive).
    pub end_byte: usize,
    /// Start line (1-indexed).
    pub start_line: usize,
    /// Start column (1-indexed).
    pub start_col: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// End column (1-indexed).
    pub end_col: usize,
}

impl Span {
    /// Create a span from a tree-sitter node.
    pub fn from_node(node: tree_sitter::Node) -> Self {
        let start = node.start_position();
        let end = node.end_position();
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: start.row + 1, // tree-sitter is 0-indexed
            start_col: start.column + 1,
            end_line: end.row + 1,
            end_col: end.column + 1,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_col)
    }
}

/// Kind of declaration (function, method, type, constant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    Function,
    Method,
    Type,
    Const,
    Interface,
    Struct,
    Enum,
    Trait,
}

impl DeclarationKind {
    /// Convert to a string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            DeclarationKind::Function => "function",
            DeclarationKind::Method => "method",
            DeclarationKind::Type => "type",
            DeclarationKind::Const => "const",
            DeclarationKind::Interface => "interface",
            DeclarationKind::Struct => "struct",
            DeclarationKind::Enum => "enum",
            DeclarationKind::Trait => "trait",
        }
    }

    /// Check if this is a callable (function or method).
    pub fn is_callable(&self) -> bool {
        matches!(self, DeclarationKind::Function | DeclarationKind::Method)
    }
}

impl fmt::Display for DeclarationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A declaration extracted from source code.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// The declaration name.
    pub name: String,
    /// The kind of declaration.
    pub kind: DeclarationKind,
    /// Source span for the entire declaration.
    pub span: Span,
    /// For methods: the receiver type (e.g., "Config" for `func (c *Config) Validate()`).
    pub receiver: Option<String>,
    /// Function body information (only for functions/methods).
    pub body: Option<FunctionBody>,
}

impl Declaration {
    /// Get the fully qualified name (receiver.name for methods).
    pub fn qualified_name(&self) -> String {
        if let Some(ref recv) = self.receiver {
            format!("{}.{}", recv, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Information about a function/method body for stub detection.
#[derive(Debug, Clone)]
pub struct FunctionBody {
    /// Span of the body block.
    pub span: Span,
    /// Number of statements in the body.
    pub statement_count: usize,
    /// Whether the body is empty (no statements).
    pub is_empty: bool,
    /// Whether the body only contains a panic/unimplemented call.
    pub is_panic_only: bool,
    /// Whether the body only returns nil/None/null.
    pub is_nil_return_only: bool,
    /// Whether the body only contains a TODO comment.
    pub has_only_todo_comment: bool,
    /// Raw text of the body (for detailed analysis).
    pub text: String,
    /// Control flow information for complexity.
    pub control_flow: ControlFlowInfo,
}

/// Control flow information for cyclomatic complexity calculation.
#[derive(Debug, Clone, Default)]
pub struct ControlFlowInfo {
    /// Number of if statements.
    pub if_count: usize,
    /// Number of for/while/loop statements.
    pub loop_count: usize,
    /// Number of switch/match statements.
    pub switch_count: usize,
    /// Number of case clauses.
    pub case_count: usize,
    /// Number of select statements (Go).
    pub select_count: usize,
    /// Number of && operators.
    pub and_count: usize,
    /// Number of || operators.
    pub or_count: usize,
    /// Number of ternary ?: operators.
    pub ternary_count: usize,
    /// Number of catch/except clauses.
    pub catch_count: usize,
}

impl ControlFlowInfo {
    /// Calculate cyclomatic complexity.
    ///
    /// CC = 1 + decision_points
    /// Decision points: if, for, while, case, &&, ||, ?, catch
    pub fn cyclomatic_complexity(&self) -> i32 {
        let decision_points = self.if_count
            + self.loop_count
            + self.case_count
            + self.select_count
            + self.and_count
            + self.or_count
            + self.ternary_count
            + self.catch_count;

        1 + decision_points as i32
    }
}

/// An import/dependency declaration.
#[derive(Debug, Clone)]
pub struct Import {
    /// The import path or module name.
    pub path: String,
    /// Optional alias (e.g., `import foo "bar"` -> alias is "foo").
    pub alias: Option<String>,
    /// Source span.
    pub span: Span,
}

/// All facts extracted from a single file.
#[derive(Debug, Clone)]
pub struct FileFacts {
    /// File path.
    pub path: String,
    /// Language identifier.
    pub language: String,
    /// Package/module name (if applicable).
    pub package: Option<String>,
    /// All declarations in the file.
    pub declarations: Vec<Declaration>,
    /// All imports in the file.
    pub imports: Vec<Import>,
    /// Whether the file had parse errors.
    pub has_parse_errors: bool,
    /// Parse error message (if any).
    pub parse_error: Option<String>,
}

impl FileFacts {
    /// Create empty facts for a file.
    pub fn empty(path: &str, language: &str) -> Self {
        Self {
            path: path.to_string(),
            language: language.to_string(),
            package: None,
            declarations: Vec::new(),
            imports: Vec::new(),
            has_parse_errors: false,
            parse_error: None,
        }
    }

    /// Find a declaration by name.
    pub fn find_declaration(&self, name: &str) -> Option<&Declaration> {
        self.declarations.iter().find(|d| d.name == name)
    }

    /// Find declarations by kind.
    pub fn declarations_by_kind(&self, kind: DeclarationKind) -> impl Iterator<Item = &Declaration> {
        self.declarations.iter().filter(move |d| d.kind == kind)
    }

    /// Get all functions and methods.
    pub fn callables(&self) -> impl Iterator<Item = &Declaration> {
        self.declarations.iter().filter(|d| d.kind.is_callable())
    }

    /// Get total cyclomatic complexity of all functions.
    pub fn total_complexity(&self) -> i32 {
        self.callables()
            .filter_map(|d| d.body.as_ref())
            .map(|b| b.control_flow.cyclomatic_complexity())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cyclomatic_complexity() {
        let mut cf = ControlFlowInfo::default();
        assert_eq!(cf.cyclomatic_complexity(), 1); // Base complexity

        cf.if_count = 2;
        cf.loop_count = 1;
        cf.and_count = 1;
        // 1 + 2 + 1 + 1 = 5
        assert_eq!(cf.cyclomatic_complexity(), 5);
    }

    #[test]
    fn test_declaration_qualified_name() {
        let func = Declaration {
            name: "main".to_string(),
            kind: DeclarationKind::Function,
            span: Span {
                start_byte: 0,
                end_byte: 10,
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 11,
            },
            receiver: None,
            body: None,
        };
        assert_eq!(func.qualified_name(), "main");

        let method = Declaration {
            name: "Validate".to_string(),
            kind: DeclarationKind::Method,
            span: Span {
                start_byte: 0,
                end_byte: 10,
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 11,
            },
            receiver: Some("Config".to_string()),
            body: None,
        };
        assert_eq!(method.qualified_name(), "Config.Validate");
    }
}

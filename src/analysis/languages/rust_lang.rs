//! Rust language analyzer using tree-sitter.
//!
//! Extracts:
//! - Function declarations
//! - Impl methods
//! - Struct/enum/trait definitions
//! - Constant declarations
//! - Use statements (imports)
//! - Control flow for complexity
//! - Function body details for stub detection

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

/// Tree-sitter query for extracting Rust declarations.
const DECLARATION_QUERY: &str = r#"
; Function declarations
(function_item
  name: (identifier) @func_name
) @function

; Methods in impl blocks
(impl_item
  type: (type_identifier) @impl_type
  body: (declaration_list
    (function_item
      name: (identifier) @method_name
    ) @method
  )
)

; Struct declarations
(struct_item
  name: (type_identifier) @struct_name
) @struct

; Enum declarations
(enum_item
  name: (type_identifier) @enum_name
) @enum

; Trait declarations
(trait_item
  name: (type_identifier) @trait_name
) @trait

; Type aliases
(type_item
  name: (type_identifier) @type_name
) @type_alias

; Constants
(const_item
  name: (identifier) @const_name
) @const

; Static items
(static_item
  name: (identifier) @static_name
) @static
"#;

/// Tree-sitter query for extracting imports (use statements).
const IMPORT_QUERY: &str = r#"
(use_declaration
  argument: (scoped_identifier) @path
) @use

(use_declaration
  argument: (use_as_clause
    path: (scoped_identifier) @path
    alias: (identifier) @alias
  )
) @use_alias

(use_declaration
  argument: (identifier) @simple_path
) @use_simple
"#;

/// Tree-sitter query for control flow nodes (complexity calculation).
const CONTROL_FLOW_QUERY: &str = r#"
(if_expression) @if
(for_expression) @for
(while_expression) @while
(loop_expression) @loop
(match_expression) @match
(match_arm) @match_arm
(binary_expression operator: "&&") @and
(binary_expression operator: "||") @or
(try_expression) @try
"#;

/// Rust language analyzer.
pub struct RustAnalyzer {
    language: Language,
}

impl RustAnalyzer {
    /// Create a new Rust analyzer.
    pub fn new() -> Self {
        Self {
            language: tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// Create a new parser for this thread.
    fn create_parser(&self) -> anyhow::Result<Parser> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)?;
        Ok(parser)
    }

    /// Extract declarations from a parsed file.
    fn extract_declarations(&self, parsed: &ParsedFile) -> anyhow::Result<Vec<Declaration>> {
        let query = Query::new(&self.language, DECLARATION_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        let mut declarations = Vec::new();
        let mut seen_positions = std::collections::HashSet::new();
        let mut current_impl_type: Option<String> = None;

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut kind = DeclarationKind::Function;
            let mut decl_node = None;
            let mut receiver = None;

            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                match capture_name {
                    "impl_type" => {
                        current_impl_type = Some(parsed.node_text(capture.node).to_string());
                    }
                    "func_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Function;
                    }
                    "method_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Method;
                        receiver = current_impl_type.clone();
                    }
                    "struct_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Struct;
                    }
                    "enum_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Enum;
                    }
                    "trait_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Trait;
                    }
                    "type_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "const_name" | "static_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Const;
                    }
                    "function" | "method" | "struct" | "enum" | "trait" | "type_alias" | "const" | "static" => {
                        decl_node = Some(capture.node);
                    }
                    _ => {}
                }
            }

            if !name.is_empty() {
                if let Some(node) = decl_node {
                    let pos_key = (node.start_byte(), name.clone());
                    if seen_positions.contains(&pos_key) {
                        continue;
                    }
                    seen_positions.insert(pos_key);

                    let body = if kind.is_callable() {
                        self.extract_function_body(parsed, node)?
                    } else {
                        None
                    };

                    declarations.push(Declaration {
                        name,
                        kind,
                        span: Span::from_node(node),
                        receiver,
                        body,
                    });
                }
            }
        }

        // Sort by position for deterministic output
        declarations.sort_by_key(|d| (d.span.start_byte, d.name.clone()));

        Ok(declarations)
    }

    /// Extract function body information for stub detection.
    fn extract_function_body(
        &self,
        parsed: &ParsedFile,
        func_node: tree_sitter::Node,
    ) -> anyhow::Result<Option<FunctionBody>> {
        // Find the body block within the function
        let body_node = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "block");

        let body_node = match body_node {
            Some(n) => n,
            None => return Ok(None),
        };

        let body_text = parsed.node_text(body_node).to_string();
        let span = Span::from_node(body_node);

        // Count statements
        let statement_count = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .count();

        // Analyze body contents
        let raw_is_empty = statement_count == 0;

        // Check for panic/unimplemented/todo! macro
        let raw_is_panic_only = self.is_panic_only_body(parsed, body_node);

        // Check for legitimate Rust patterns that should NOT be flagged as stubs
        let func_text = parsed.node_text(func_node);
        let is_legitimate_pattern = self.is_legitimate_rust_pattern(
            parsed, func_node, func_text, raw_is_empty, raw_is_panic_only
        );

        // Only flag as empty/panic if not a legitimate pattern
        let is_empty = raw_is_empty && !is_legitimate_pattern;
        let is_panic_only = raw_is_panic_only && !is_legitimate_pattern;

        // Check for None return
        let is_nil_return_only = self.is_none_return_only_body(parsed, body_node);

        // Check for TODO-only body
        let has_only_todo_comment = self.has_only_todo_comment(parsed, body_node);

        // Extract control flow for complexity
        let control_flow = self.extract_control_flow(parsed, body_node)?;

        Ok(Some(FunctionBody {
            span,
            statement_count,
            is_empty,
            is_panic_only,
            is_nil_return_only,
            has_only_todo_comment,
            text: body_text,
            control_flow,
        }))
    }

    /// Check if this is a legitimate Rust pattern that should not be flagged as a stub.
    ///
    /// Legitimate patterns include:
    /// - Compile-time trait bound verification functions (check_send, is_unpin, etc.)
    /// - Conditional compilation no-ops (metrics disabled, etc.)
    /// - Platform compatibility stubs in stub.rs files
    /// - Default trait implementations
    fn is_legitimate_rust_pattern(
        &self,
        parsed: &ParsedFile,
        func_node: tree_sitter::Node,
        func_text: &str,
        is_empty: bool,
        is_panic_only: bool,
    ) -> bool {
        // Extract function name
        let func_name = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "identifier")
            .map(|n| parsed.node_text(n))
            .unwrap_or("");

        // Check file path for stub/mock files
        let path_lower = parsed.path.to_lowercase();
        let is_stub_file = path_lower.ends_with("stub.rs")
            || path_lower.ends_with("stubs.rs")
            || path_lower.ends_with("mock.rs")
            || path_lower.ends_with("mocks.rs")
            || path_lower.ends_with("noop.rs")
            || path_lower.contains("/stub/")
            || path_lower.contains("/mock/");

        // Platform stubs that panic are intentional
        if is_panic_only && is_stub_file {
            return true;
        }

        // Empty functions in stub/mock files are intentional no-ops
        if is_empty && is_stub_file {
            return true;
        }

        // Check for compile-time trait bound verification patterns
        // These are empty functions that verify trait bounds at compile time
        if is_empty {
            // Common trait verification function names
            let trait_check_names = [
                "check_send", "check_sync", "check_unpin", "check_static",
                "check_send_sync", "check_send_sync_val", "check_static_val",
                "is_send", "is_sync", "is_unpin", "is_debug",
                "_assert", "_assert_send", "_assert_sync", "_assert_unpin",
            ];
            if trait_check_names.iter().any(|&name| func_name == name || func_name.starts_with(name)) {
                return true;
            }

            // Functions with generic type parameters and trait bounds are likely compile-time checks
            // e.g., fn check_send<T: Send>() {}
            if func_text.contains("<") && func_text.contains(":") {
                // Has generic with bounds - likely a trait check
                if func_name.starts_with("check_") || func_name.starts_with("is_") || func_name.starts_with("_") {
                    return true;
                }
            }

            // No-op counter/metric functions (often conditionally compiled)
            let noop_prefixes = ["inc_", "dec_", "add_", "record_", "log_"];
            if noop_prefixes.iter().any(|p| func_name.starts_with(p)) {
                // Check if this looks like a metrics/counter file
                if path_lower.contains("counter")
                    || path_lower.contains("metric")
                    || path_lower.contains("stats")
                {
                    return true;
                }
            }

            // Default trait implementations (initialize, finalize, etc.)
            let default_impl_names = [
                "initialize", "finalize", "default", "new",
                "consume", "flush", "clear", "reset",
                "wake", "drop", "close",
            ];
            // Only skip if the function is very simple (no params or just &self)
            let simple_signature = !func_text.contains(",") || func_text.contains("&self)") || func_text.contains("&mut self)");
            if default_impl_names.iter().any(|&name| func_name == name) && simple_signature {
                return true;
            }

            // Functions starting with underscore are often intentionally unused/placeholder
            if func_name.starts_with("_") {
                return true;
            }

            // retain_ready and similar callback functions
            if func_name.contains("retain") || func_name.contains("callback") || func_name.contains("handler") {
                return true;
            }

            // post_* and pre_* hook functions that may be no-ops
            if func_name.starts_with("post_") || func_name.starts_with("pre_") || func_name.starts_with("on_") {
                return true;
            }

            // unhandled_* functions are often intentional no-ops
            if func_name.starts_with("unhandled_") {
                return true;
            }

            // Functions with explicitly unused parameters (prefixed with _) are intentional no-ops
            // e.g., fn initialize(_: Internal, _lower: usize) {}
            // These are callbacks where the implementation doesn't need the params
            if func_text.contains("_:") || func_text.contains("_,") || func_text.contains(", _)") {
                return true;
            }

            // Methods on types that are intentionally empty (Empty, Noop, etc.)
            let empty_type_names = ["Empty", "Noop", "NoOp", "Void", "Unit", "Null", "Dummy"];
            if empty_type_names.iter().any(|t| path_lower.contains(&t.to_lowercase())) {
                return true;
            }
        }

        false
    }

    /// Check if a function body only contains a panic/unimplemented/todo! macro.
    fn is_panic_only_body(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        let text = parsed.node_text(stmt);

        // Check for panic!, unimplemented!, todo! macros
        if stmt.kind() == "expression_statement" || stmt.kind() == "macro_invocation" {
            let trimmed = text.trim();

            // Only flag as stub if it's unimplemented!/todo! OR panic!() without a message
            // panic!("some message") is often an intentional error handler
            if trimmed.starts_with("unimplemented!") || trimmed.starts_with("todo!") {
                return true;
            }

            // For panic!, only flag if it's empty panic!() without a meaningful message
            if trimmed.starts_with("panic!") {
                // panic!() or panic!("") are stubs
                // panic!("meaningful error message") are intentional error handlers
                let is_empty_panic = trimmed == "panic!()"
                    || trimmed == "panic!();"
                    || trimmed == r#"panic!("")"#
                    || trimmed == r#"panic!("");"#;
                return is_empty_panic;
            }
        }

        false
    }

    /// Check if a function body only returns None.
    fn is_none_return_only_body(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        let text = parsed.node_text(stmt).trim();

        // Check for `None` or `return None`
        text == "None" || text == "return None" || text == "return None;"
    }

    /// Check if a function body only contains a TODO comment.
    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "{" | "}" => continue,
                "line_comment" | "block_comment" => {
                    let text = parsed.node_text(child).to_uppercase();
                    if text.contains("TODO") || text.contains("FIXME") || text.contains("UNIMPLEMENTED") {
                        has_todo = true;
                    }
                }
                _ => {
                    has_other = true;
                }
            }
        }

        has_todo && !has_other
    }

    /// Extract control flow information from a function body.
    fn extract_control_flow(
        &self,
        parsed: &ParsedFile,
        body_node: tree_sitter::Node,
    ) -> anyhow::Result<ControlFlowInfo> {
        let query = Query::new(&self.language, CONTROL_FLOW_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, body_node, &parsed.source[..]);

        let mut info = ControlFlowInfo::default();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                match name {
                    "if" => info.if_count += 1,
                    "for" | "while" | "loop" => info.loop_count += 1,
                    "match" => info.switch_count += 1,
                    "match_arm" => info.case_count += 1,
                    "and" => info.and_count += 1,
                    "or" => info.or_count += 1,
                    "try" => info.catch_count += 1, // ? operator adds a branch
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    /// Extract imports from a parsed file.
    fn extract_imports(&self, parsed: &ParsedFile) -> anyhow::Result<Vec<Import>> {
        let query = Query::new(&self.language, IMPORT_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        let mut imports = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        while let Some(m) = matches.next() {
            let mut path = String::new();
            let mut alias = None;
            let mut import_node = None;

            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                match name {
                    "path" | "simple_path" => {
                        path = parsed.node_text(capture.node).to_string();
                        import_node = Some(capture.node);
                    }
                    "alias" => {
                        alias = Some(parsed.node_text(capture.node).to_string());
                    }
                    _ => {}
                }
            }

            if !path.is_empty() && !seen_paths.contains(&path) {
                seen_paths.insert(path.clone());
                if let Some(node) = import_node {
                    imports.push(Import {
                        path,
                        alias,
                        span: Span::from_node(node),
                    });
                }
            }
        }

        // Sort by path for deterministic output
        imports.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(imports)
    }
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for RustAnalyzer {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.rs"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Rust source: {}", path.display()))?;

        Ok(ParsedFile {
            tree,
            source: source.to_vec(),
            path: path.to_string_lossy().to_string(),
        })
    }

    fn extract_facts(&self, parsed: &ParsedFile) -> anyhow::Result<FileFacts> {
        let declarations = self.extract_declarations(parsed)?;
        let imports = self.extract_imports(parsed)?;

        // Check for parse errors
        let has_parse_errors = parsed.tree.root_node().has_error();
        let parse_error = if has_parse_errors {
            Some("Source contains syntax errors".to_string())
        } else {
            None
        };

        Ok(FileFacts {
            path: parsed.path.clone(),
            language: self.language_id().to_string(),
            package: None, // Rust uses mod system, not packages
            declarations,
            imports,
            has_parse_errors,
            parse_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> (RustAnalyzer, ParsedFile) {
        let analyzer = RustAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("test.rs"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"
fn main() {
    println!("hello");
}

pub fn helper(x: i32) -> i32 {
    x + 1
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert_eq!(facts.declarations.len(), 2);
        assert!(facts.declarations.iter().any(|d| d.name == "main" && d.kind == DeclarationKind::Function));
        assert!(facts.declarations.iter().any(|d| d.name == "helper" && d.kind == DeclarationKind::Function));
    }

    #[test]
    fn test_extract_methods() {
        let source = r#"
struct Config {
    name: String,
}

impl Config {
    fn new(name: String) -> Self {
        Self { name }
    }

    fn validate(&self) -> Result<(), Error> {
        Ok(())
    }
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let methods: Vec<_> = facts.declarations.iter()
            .filter(|d| d.kind == DeclarationKind::Method)
            .collect();

        assert_eq!(methods.len(), 2);
        assert!(methods.iter().any(|d| d.name == "new" && d.receiver == Some("Config".to_string())));
        assert!(methods.iter().any(|d| d.name == "validate" && d.receiver == Some("Config".to_string())));
    }

    #[test]
    fn test_extract_types() {
        let source = r#"
struct Handler {
    name: String,
}

enum Status {
    Active,
    Inactive,
}

trait Service {
    fn run(&self) -> Result<(), Error>;
}

type Id = String;
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "Handler" && d.kind == DeclarationKind::Struct));
        assert!(facts.declarations.iter().any(|d| d.name == "Status" && d.kind == DeclarationKind::Enum));
        assert!(facts.declarations.iter().any(|d| d.name == "Service" && d.kind == DeclarationKind::Trait));
        assert!(facts.declarations.iter().any(|d| d.name == "Id" && d.kind == DeclarationKind::Type));
    }

    #[test]
    fn test_extract_constants() {
        let source = r#"
const VERSION: &str = "1.0.0";
const MAX_RETRIES: u32 = 3;
static INSTANCE: Lazy<Config> = Lazy::new(|| Config::default());
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let consts: Vec<_> = facts.declarations.iter()
            .filter(|d| d.kind == DeclarationKind::Const)
            .collect();

        assert_eq!(consts.len(), 3);
        assert!(consts.iter().any(|d| d.name == "VERSION"));
        assert!(consts.iter().any(|d| d.name == "MAX_RETRIES"));
        assert!(consts.iter().any(|d| d.name == "INSTANCE"));
    }

    #[test]
    fn test_complexity_simple() {
        let source = r#"
fn simple() -> i32 {
    42
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("simple").unwrap();
        let body = func.body.as_ref().unwrap();
        assert_eq!(body.control_flow.cyclomatic_complexity(), 1);
    }

    #[test]
    fn test_complexity_with_branches() {
        let source = r#"
fn branchy(x: i32) -> i32 {
    if x > 0 {
        if x > 10 {
            100
        } else {
            10
        }
    } else {
        0
    }
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("branchy").unwrap();
        let body = func.body.as_ref().unwrap();
        // 1 base + 2 if = 3
        assert_eq!(body.control_flow.cyclomatic_complexity(), 3);
    }

    #[test]
    fn test_complexity_with_match() {
        let source = r#"
fn matcher(x: Option<i32>) -> i32 {
    match x {
        Some(v) if v > 0 => v,
        Some(_) => 0,
        None => -1,
    }
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("matcher").unwrap();
        let body = func.body.as_ref().unwrap();
        // 1 base + 1 match + 3 arms = 5
        assert!(body.control_flow.cyclomatic_complexity() >= 4);
    }

    #[test]
    fn test_stub_detection_empty() {
        let source = r#"
fn empty() {
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("empty").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_empty);
    }

    #[test]
    fn test_stub_detection_unimplemented() {
        let source = r#"
fn not_implemented() {
    unimplemented!()
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("not_implemented").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_panic_only);
    }

    #[test]
    fn test_stub_detection_todo_macro() {
        let source = r#"
fn placeholder() {
    todo!()
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("placeholder").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_panic_only);
    }

    #[test]
    fn test_stub_detection_todo_comment() {
        let source = r#"
fn with_todo() {
    // TODO: implement this
}
"#;
        let (analyzer, parsed) = parse_rust(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("with_todo").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.has_only_todo_comment);
    }
}

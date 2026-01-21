//! Go language analyzer using tree-sitter.
//!
//! Extracts:
//! - Function declarations (including methods with receivers)
//! - Type declarations (struct, interface, type aliases)
//! - Constant declarations
//! - Imports
//! - Control flow for complexity
//! - Function body details for stub detection

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

/// Tree-sitter query for extracting Go declarations.
const DECLARATION_QUERY: &str = r#"
; Function declarations
(function_declaration
  name: (identifier) @func_name
) @function

; Method declarations (with receiver)
(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: [
        (pointer_type (type_identifier) @receiver_type)
        (type_identifier) @receiver_type
      ]
    )
  )
  name: (field_identifier) @method_name
) @method

; Type declarations
(type_declaration
  (type_spec
    name: (type_identifier) @type_name
    type: (struct_type)
  )
) @struct

(type_declaration
  (type_spec
    name: (type_identifier) @type_name
    type: (interface_type)
  )
) @interface

(type_declaration
  (type_spec
    name: (type_identifier) @type_name
    type: (_) @other_type
  )
) @type_alias

; Constant declarations
(const_declaration
  (const_spec
    name: (identifier) @const_name
  )
) @const
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
(import_declaration
  (import_spec
    name: (package_identifier)? @alias
    path: (interpreted_string_literal) @path
  )
) @import

(import_declaration
  (import_spec_list
    (import_spec
      name: (package_identifier)? @alias
      path: (interpreted_string_literal) @path
    ) @import_item
  )
) @import_group
"#;

/// Tree-sitter query for package declaration.
const PACKAGE_QUERY: &str = r#"
(package_clause
  (package_identifier) @package_name
)
"#;

/// Tree-sitter query for control flow nodes (complexity calculation).
const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(expression_switch_statement) @switch
(type_switch_statement) @switch
(select_statement) @select
(communication_case) @case
(expression_case) @case
(type_case) @case
(default_case) @default_case
(binary_expression operator: "&&") @and
(binary_expression operator: "||") @or
"#;


/// Go language analyzer.
pub struct GoAnalyzer {
    language: Language,
}

impl GoAnalyzer {
    /// Create a new Go analyzer.
    pub fn new() -> Self {
        Self {
            language: tree_sitter_go::LANGUAGE.into(),
        }
    }

    /// Create a new parser for this thread.
    fn create_parser(&self) -> anyhow::Result<Parser> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)?;
        Ok(parser)
    }

    /// Extract the package name from a parsed file.
    fn extract_package(&self, parsed: &ParsedFile) -> Option<String> {
        let query = Query::new(&self.language, PACKAGE_QUERY).ok()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        if let Some(m) = matches.next() {
            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                if name == "package_name" {
                    return Some(parsed.node_text(capture.node).to_string());
                }
            }
        }
        None
    }

    /// Extract declarations from a parsed file.
    fn extract_declarations(&self, parsed: &ParsedFile) -> anyhow::Result<Vec<Declaration>> {
        let query = Query::new(&self.language, DECLARATION_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        let mut declarations = Vec::new();
        let mut seen_positions = std::collections::HashSet::new();

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut kind = DeclarationKind::Function;
            let mut decl_node = None;
            let mut receiver = None;

            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                match capture_name {
                    "func_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Function;
                    }
                    "method_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Method;
                    }
                    "receiver_type" => {
                        receiver = Some(parsed.node_text(capture.node).to_string());
                    }
                    "type_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        // Don't set kind here - let the struct/interface/type_alias capture set it
                    }
                    "const_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Const;
                    }
                    "function" | "method" => {
                        decl_node = Some(capture.node);
                    }
                    "struct" => {
                        decl_node = Some(capture.node);
                        kind = DeclarationKind::Struct;
                    }
                    "interface" => {
                        decl_node = Some(capture.node);
                        kind = DeclarationKind::Interface;
                    }
                    "type_alias" => {
                        decl_node = Some(capture.node);
                        kind = DeclarationKind::Type;
                    }
                    "const" => {
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
                        self.extract_function_body(parsed, &name, node)?
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
        _func_name: &str,
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
            .filter(|n| {
                !matches!(n.kind(), "{" | "}" | "comment")
            })
            .count();

        // Analyze body contents
        let is_empty = statement_count == 0;

        // Check for panic-only body
        let is_panic_only = self.is_panic_only_body(parsed, body_node);

        // Check for nil-return-only body
        let is_nil_return_only = self.is_nil_return_only_body(parsed, body_node);

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

    /// Check if a function body only contains a panic call.
    fn is_panic_only_body(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];

        // Check for expression_statement containing call_expression
        if stmt.kind() == "expression_statement" {
            if let Some(expr) = stmt.child(0) {
                if expr.kind() == "call_expression" {
                    if let Some(func) = expr.child_by_field_name("function") {
                        let func_name = parsed.node_text(func);
                        return func_name == "panic";
                    }
                }
            }
        }

        false
    }

    /// Check if a function body only returns nil.
    fn is_nil_return_only_body(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];

        if stmt.kind() == "return_statement" {
            // Check if all return values are nil
            let return_values: Vec<_> = stmt
                .children(&mut stmt.walk())
                .filter(|n| n.kind() != "return")
                .collect();

            if return_values.is_empty() {
                // Empty return - could be for void functions
                return false;
            }

            // Check for expression_list with nil values
            for child in stmt.children(&mut stmt.walk()) {
                if child.kind() == "expression_list" {
                    let exprs: Vec<_> = child.children(&mut child.walk()).collect();
                    return exprs.iter().all(|e| {
                        e.kind() == "nil" || (e.kind() == "," || parsed.node_text(*e).trim() == "nil")
                    });
                }
                if child.kind() == "nil" {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a function body only contains a TODO comment.
    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "{" | "}" => continue,
                "comment" => {
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
                    "for" => info.loop_count += 1,
                    "switch" => info.switch_count += 1,
                    "select" => info.select_count += 1,
                    "case" => info.case_count += 1,
                    "and" => info.and_count += 1,
                    "or" => info.or_count += 1,
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
                    "path" => {
                        // Remove quotes from path
                        let raw = parsed.node_text(capture.node);
                        path = raw.trim_matches('"').to_string();
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

impl Default for GoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for GoAnalyzer {
    fn language_id(&self) -> &'static str {
        "go"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.go"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Go source: {}", path.display()))?;

        Ok(ParsedFile {
            tree,
            source: source.to_vec(),
            path: path.to_string_lossy().to_string(),
        })
    }

    fn extract_facts(&self, parsed: &ParsedFile) -> anyhow::Result<FileFacts> {
        let package = self.extract_package(parsed);
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
            package,
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

    fn parse_go(source: &str) -> (GoAnalyzer, ParsedFile) {
        let analyzer = GoAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("test.go"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_package() {
        let (analyzer, parsed) = parse_go("package main\n");
        let pkg = analyzer.extract_package(&parsed);
        assert_eq!(pkg, Some("main".to_string()));
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"
package main

func main() {
    println("hello")
}

func helper(x int) int {
    return x + 1
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert_eq!(facts.declarations.len(), 2);
        assert!(facts.declarations.iter().any(|d| d.name == "main" && d.kind == DeclarationKind::Function));
        assert!(facts.declarations.iter().any(|d| d.name == "helper" && d.kind == DeclarationKind::Function));
    }

    #[test]
    fn test_extract_methods() {
        let source = r#"
package main

type Config struct {
    Name string
}

func (c *Config) Validate() error {
    return nil
}

func (c Config) String() string {
    return c.Name
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let methods: Vec<_> = facts.declarations.iter()
            .filter(|d| d.kind == DeclarationKind::Method)
            .collect();

        assert_eq!(methods.len(), 2);

        let validate = methods.iter().find(|d| d.name == "Validate").unwrap();
        assert_eq!(validate.receiver, Some("Config".to_string()));

        let string_method = methods.iter().find(|d| d.name == "String").unwrap();
        assert_eq!(string_method.receiver, Some("Config".to_string()));
    }

    #[test]
    fn test_extract_types() {
        let source = r#"
package main

type Handler struct {
    name string
}

type Service interface {
    Run() error
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let types: Vec<_> = facts.declarations.iter()
            .filter(|d| matches!(d.kind, DeclarationKind::Type | DeclarationKind::Struct | DeclarationKind::Interface))
            .collect();

        // Handler (struct) and Service (interface)
        assert_eq!(types.len(), 2);
        assert!(types.iter().any(|d| d.name == "Handler" && d.kind == DeclarationKind::Struct));
        assert!(types.iter().any(|d| d.name == "Service" && d.kind == DeclarationKind::Interface));
    }

    #[test]
    fn test_extract_constants() {
        let source = r#"
package main

const Version = "1.0.0"

const (
    MaxRetries = 3
    Timeout = 30
)
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let consts: Vec<_> = facts.declarations.iter()
            .filter(|d| d.kind == DeclarationKind::Const)
            .collect();

        assert_eq!(consts.len(), 3);
        assert!(consts.iter().any(|d| d.name == "Version"));
        assert!(consts.iter().any(|d| d.name == "MaxRetries"));
        assert!(consts.iter().any(|d| d.name == "Timeout"));
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
package main

import (
    "fmt"
    "os"
    log "github.com/sirupsen/logrus"
)
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert_eq!(facts.imports.len(), 3);
        assert!(facts.imports.iter().any(|i| i.path == "fmt" && i.alias.is_none()));
        assert!(facts.imports.iter().any(|i| i.path == "os"));
        assert!(facts.imports.iter().any(|i| i.path == "github.com/sirupsen/logrus" && i.alias == Some("log".to_string())));
    }

    #[test]
    fn test_complexity_simple() {
        let source = r#"
package main

func simple() int {
    return 42
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("simple").unwrap();
        let body = func.body.as_ref().unwrap();
        assert_eq!(body.control_flow.cyclomatic_complexity(), 1);
    }

    #[test]
    fn test_complexity_with_branches() {
        let source = r#"
package main

func branchy(x int) int {
    if x > 0 {
        if x > 10 {
            return 100
        }
        return 10
    }
    return 0
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("branchy").unwrap();
        let body = func.body.as_ref().unwrap();
        // 1 base + 2 if statements = 3
        assert_eq!(body.control_flow.cyclomatic_complexity(), 3);
    }

    #[test]
    fn test_complexity_with_loops_and_logic() {
        let source = r#"
package main

func complex(items []int) int {
    sum := 0
    for _, item := range items {
        if item > 0 && item < 100 {
            sum += item
        }
    }
    return sum
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("complex").unwrap();
        let body = func.body.as_ref().unwrap();
        // 1 base + 1 for + 1 if + 1 && = 4
        assert_eq!(body.control_flow.cyclomatic_complexity(), 4);
    }

    #[test]
    fn test_stub_detection_empty() {
        let source = r#"
package main

func empty() {
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("empty").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_empty);
    }

    #[test]
    fn test_stub_detection_panic() {
        let source = r#"
package main

func notImplemented() {
    panic("not implemented")
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("notImplemented").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_panic_only);
    }

    #[test]
    fn test_stub_detection_nil_return() {
        let source = r#"
package main

func stubbed() error {
    return nil
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("stubbed").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.is_nil_return_only);
    }

    #[test]
    fn test_stub_detection_todo_comment() {
        let source = r#"
package main

func placeholder() {
    // TODO: implement this
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("placeholder").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(body.has_only_todo_comment);
    }

    #[test]
    fn test_not_stub_real_implementation() {
        let source = r#"
package main

func realFunc(x int) int {
    if x > 0 {
        return x * 2
    }
    return 0
}
"#;
        let (analyzer, parsed) = parse_go(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let func = facts.find_declaration("realFunc").unwrap();
        let body = func.body.as_ref().unwrap();
        assert!(!body.is_empty);
        assert!(!body.is_panic_only);
        assert!(!body.is_nil_return_only);
        assert!(!body.has_only_todo_comment);
    }
}

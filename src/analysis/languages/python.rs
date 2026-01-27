//! Python language analyzer using tree-sitter.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

const DECLARATION_QUERY: &str = r#"
; Function definitions
(function_definition
  name: (identifier) @func_name
) @function

; Class definitions
(class_definition
  name: (identifier) @class_name
) @class

; Decorated function definitions
(decorated_definition
  (function_definition
    name: (identifier) @decorated_func_name
  )
) @decorated_function

; Decorated class definitions
(decorated_definition
  (class_definition
    name: (identifier) @decorated_class_name
  )
) @decorated_class
"#;

const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(while_statement) @while
(conditional_expression) @ternary
(boolean_operator operator: "and") @and
(boolean_operator operator: "or") @or
(try_statement) @try
(except_clause) @except
(match_statement) @match
(case_clause) @case
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
; import module
(import_statement
  name: (dotted_name) @module_name
) @import

; from module import name
(import_from_statement
  module_name: (dotted_name) @from_module
) @import_from

; from . import name (relative imports)
(import_from_statement
  module_name: (relative_import) @relative_module
) @import_relative
"#;

pub struct PythonAnalyzer {
    language: Language,
}

impl PythonAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_python::LANGUAGE.into(),
        }
    }

    fn create_parser(&self) -> anyhow::Result<Parser> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)?;
        Ok(parser)
    }

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

            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                match capture_name {
                    "func_name" | "decorated_func_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Function;
                    }
                    "class_name" | "decorated_class_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "function" | "decorated_function" | "class" | "decorated_class" => {
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

                    let body = if kind == DeclarationKind::Function {
                        self.extract_function_body(parsed, node)?
                    } else {
                        None
                    };

                    declarations.push(Declaration {
                        name,
                        kind,
                        span: Span::from_node(node),
                        receiver: None,
                        body,
                    });
                }
            }
        }

        declarations.sort_by_key(|d| (d.span.start_byte, d.name.clone()));
        Ok(declarations)
    }

    fn extract_function_body(
        &self,
        parsed: &ParsedFile,
        func_node: tree_sitter::Node,
    ) -> anyhow::Result<Option<FunctionBody>> {
        // For Python, the body is a block node
        let body_node = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "block");

        let body_node = match body_node {
            Some(n) => n,
            None => return Ok(None),
        };

        let body_text = parsed.node_text(body_node).to_string();
        let span = Span::from_node(body_node);

        let statement_count = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "comment"))
            .count();

        let is_empty = statement_count == 0;
        let control_flow = self.extract_control_flow(parsed, body_node)?;

        Ok(Some(FunctionBody {
            span,
            statement_count,
            is_empty,
            is_panic_only: self.is_raise_only(parsed, body_node),
            is_nil_return_only: self.is_none_return_only(parsed, body_node),
            has_only_todo_comment: self.has_only_todo_comment(parsed, body_node),
            text: body_text,
            control_flow,
        }))
    }

    fn is_raise_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        if stmt.kind() != "raise_statement" {
            return false;
        }

        // In Python, many functions intentionally raise exceptions as their primary behavior:
        // - Abstract methods: raise NotImplementedError()
        // - Guard functions: raise ValueError(), raise TypeError()
        // - Immutability enforcement: raise AttributeError()
        // - Deletion protection: raise ProtectedError()
        //
        // These are NOT stubs - they're the actual implementation.
        // Only flag raises that look like actual incomplete/stub code:
        // - raise Exception("not implemented")
        // - raise Exception("TODO")
        // - raise Exception() with no specific type
        let raise_text = parsed.node_text(stmt).to_lowercase();

        // Only flag generic Exception with stub-like messages
        if raise_text.contains("exception") {
            let has_stub_message = raise_text.contains("not implemented")
                || raise_text.contains("todo")
                || raise_text.contains("stub")
                || raise_text.contains("placeholder")
                || raise_text.contains("not yet");
            return has_stub_message;
        }

        // All other specific exception types (NotImplementedError, ValueError,
        // TypeError, AttributeError, etc.) are intentional behavior, not stubs
        false
    }

    fn is_none_return_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        if stmt.kind() == "return_statement" {
            let text = parsed.node_text(stmt).trim();
            return text == "return None" || text == "return";
        }
        // Check for pass statement
        if stmt.kind() == "pass_statement" {
            return true;
        }
        false
    }

    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "comment" => {
                    let text = parsed.node_text(child).to_uppercase();
                    if text.contains("TODO") || text.contains("FIXME") {
                        has_todo = true;
                    }
                }
                "pass_statement" => {
                    // pass with TODO comment is still a stub
                }
                "expression_statement" => {
                    // Check for docstring (string as first statement)
                    if let Some(first_child) = child.child(0) {
                        if first_child.kind() == "string" {
                            let text = parsed.node_text(first_child).to_uppercase();
                            if text.contains("TODO") || text.contains("FIXME") {
                                has_todo = true;
                                continue;
                            }
                        }
                    }
                    has_other = true;
                }
                _ => has_other = true,
            }
        }

        has_todo && !has_other
    }

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
                    "for" | "while" => info.loop_count += 1,
                    "match" => info.switch_count += 1,
                    "case" => info.case_count += 1,
                    "ternary" => info.ternary_count += 1,
                    "and" => info.and_count += 1,
                    "or" => info.or_count += 1,
                    "except" => info.catch_count += 1,
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    fn extract_imports(&self, parsed: &ParsedFile) -> anyhow::Result<Vec<Import>> {
        let query = Query::new(&self.language, IMPORT_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        let mut imports = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        while let Some(m) = matches.next() {
            let mut path = String::new();
            let mut import_node = None;

            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                match name {
                    "module_name" | "from_module" => {
                        path = parsed.node_text(capture.node).to_string();
                        import_node = Some(capture.node);
                    }
                    "relative_module" => {
                        // Handle relative imports like "from . import x"
                        path = parsed.node_text(capture.node).to_string();
                        import_node = Some(capture.node);
                    }
                    _ => {}
                }
            }

            if !path.is_empty() && !seen_paths.contains(&path) {
                seen_paths.insert(path.clone());
                if let Some(node) = import_node {
                    imports.push(Import {
                        path,
                        alias: None,
                        span: Span::from_node(node),
                    });
                }
            }
        }

        imports.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(imports)
    }
}

impl Default for PythonAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for PythonAnalyzer {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.py"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Python source: {}", path.display()))?;

        Ok(ParsedFile {
            tree,
            source: source.to_vec(),
            path: path.to_string_lossy().to_string(),
        })
    }

    fn extract_facts(&self, parsed: &ParsedFile) -> anyhow::Result<FileFacts> {
        let declarations = self.extract_declarations(parsed)?;
        let imports = self.extract_imports(parsed)?;

        Ok(FileFacts {
            path: parsed.path.clone(),
            language: self.language_id().to_string(),
            package: None,
            declarations,
            imports,
            has_parse_errors: parsed.tree.root_node().has_error(),
            parse_error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_python(source: &str) -> (PythonAnalyzer, ParsedFile) {
        let analyzer = PythonAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("test.py"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
import os
import sys
from collections import OrderedDict
from typing import List, Optional
from . import local_module
"#;
        let (analyzer, parsed) = parse_python(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.imports.iter().any(|i| i.path == "os"));
        assert!(facts.imports.iter().any(|i| i.path == "sys"));
        assert!(facts.imports.iter().any(|i| i.path == "collections"));
        assert!(facts.imports.iter().any(|i| i.path == "typing"));
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"
def simple():
    pass

def with_args(x, y):
    return x + y

class MyClass:
    def method(self):
        pass
"#;
        let (analyzer, parsed) = parse_python(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "simple"));
        assert!(facts.declarations.iter().any(|d| d.name == "with_args"));
        assert!(facts.declarations.iter().any(|d| d.name == "MyClass"));
    }

    #[test]
    fn test_stub_detection() {
        let source = r#"
def abstract_method():
    raise NotImplementedError()

def guard_function():
    raise ValueError("invalid input")

def immutable_guard():
    raise AttributeError("cannot modify")

def actual_stub():
    raise Exception("not implemented")

def stub_pass():
    pass

def stub_none():
    return None
"#;
        let (analyzer, parsed) = parse_python(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        // NotImplementedError is Python's abstract method pattern - NOT a stub
        let abstract_method = facts.declarations.iter().find(|d| d.name == "abstract_method").unwrap();
        assert!(!abstract_method.body.as_ref().unwrap().is_panic_only,
            "NotImplementedError should not be flagged as stub");

        // ValueError, TypeError, AttributeError etc. are intentional guard functions - NOT stubs
        let guard = facts.declarations.iter().find(|d| d.name == "guard_function").unwrap();
        assert!(!guard.body.as_ref().unwrap().is_panic_only,
            "ValueError guard should not be flagged as stub");

        let immutable = facts.declarations.iter().find(|d| d.name == "immutable_guard").unwrap();
        assert!(!immutable.body.as_ref().unwrap().is_panic_only,
            "AttributeError guard should not be flagged as stub");

        // Only generic Exception with stub-like messages ARE stubs
        let actual_stub = facts.declarations.iter().find(|d| d.name == "actual_stub").unwrap();
        assert!(actual_stub.body.as_ref().unwrap().is_panic_only,
            "Generic Exception('not implemented') should be flagged as stub");

        let stub_pass = facts.declarations.iter().find(|d| d.name == "stub_pass").unwrap();
        assert!(stub_pass.body.as_ref().unwrap().is_nil_return_only);

        let stub_none = facts.declarations.iter().find(|d| d.name == "stub_none").unwrap();
        assert!(stub_none.body.as_ref().unwrap().is_nil_return_only);
    }
}

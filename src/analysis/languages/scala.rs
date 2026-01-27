//! Scala language analyzer using tree-sitter.

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

; Value definitions (val/var with function type)
(val_definition
  pattern: (identifier) @val_name
) @val

; Class definitions
(class_definition
  name: (identifier) @class_name
) @class

; Object definitions
(object_definition
  name: (identifier) @object_name
) @object

; Trait definitions
(trait_definition
  name: (identifier) @trait_name
) @trait
"#;

const CONTROL_FLOW_QUERY: &str = r#"
(if_expression) @if
(for_expression) @for
(while_expression) @while
(match_expression) @match
(case_clause) @case
(try_expression) @try
(catch_clause) @catch
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
; import scala.collection.mutable
(import_declaration) @import

; Package declaration
(package_clause) @package
"#;

pub struct ScalaAnalyzer {
    language: Language,
}

impl ScalaAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_scala::LANGUAGE.into(),
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
                    "func_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Method;
                    }
                    "val_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Const;
                    }
                    "class_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "object_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "trait_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Interface;
                    }
                    "function" | "val" | "class" | "object" | "trait" => {
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

                    let body = if kind == DeclarationKind::Method {
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
        // Scala functions can have a block body or an expression body
        let body_node = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "block" || n.kind() == "indented_block");

        let body_node = match body_node {
            Some(n) => n,
            None => return Ok(None),
        };

        let body_text = parsed.node_text(body_node).to_string();
        let span = Span::from_node(body_node);

        let statement_count = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment"))
            .count();

        let is_empty = statement_count == 0;
        let control_flow = self.extract_control_flow(parsed, body_node)?;

        Ok(Some(FunctionBody {
            span,
            statement_count,
            is_empty,
            is_panic_only: self.is_throw_only(parsed, body_node),
            is_nil_return_only: false,
            has_only_todo_comment: self.has_only_todo_comment(parsed, body_node),
            text: body_text,
            control_flow,
        }))
    }

    fn is_throw_only(&self, _parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        stmt.kind() == "throw_expression"
    }

    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "{" | "}" => continue,
                "comment" => {
                    let text = parsed.node_text(child).to_uppercase();
                    if text.contains("TODO") || text.contains("FIXME") {
                        has_todo = true;
                    }
                }
                "call_expression" => {
                    // Check for ??? (not implemented)
                    let text = parsed.node_text(child).trim();
                    if text == "???" {
                        return true;
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
                    "catch" => info.catch_count += 1,
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    fn extract_package(&self, parsed: &ParsedFile) -> Option<String> {
        let query = Query::new(&self.language, IMPORT_QUERY).ok()?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                if name == "package" {
                    // Extract text after "package "
                    let text = parsed.node_text(capture.node).trim();
                    if let Some(pkg) = text.strip_prefix("package ") {
                        return Some(pkg.trim().to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_imports(&self, parsed: &ParsedFile) -> anyhow::Result<Vec<Import>> {
        let query = Query::new(&self.language, IMPORT_QUERY)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, parsed.tree.root_node(), &parsed.source[..]);

        let mut imports = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let name = query.capture_names()[capture.index as usize];
                if name == "import" {
                    // Extract text after "import "
                    let text = parsed.node_text(capture.node).trim();
                    if let Some(path) = text.strip_prefix("import ") {
                        let path = path.trim().to_string();
                        if !path.is_empty() && !seen_paths.contains(&path) {
                            seen_paths.insert(path.clone());
                            imports.push(Import {
                                path,
                                alias: None,
                                span: Span::from_node(capture.node),
                            });
                        }
                    }
                }
            }
        }

        imports.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(imports)
    }
}

impl Default for ScalaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for ScalaAnalyzer {
    fn language_id(&self) -> &'static str {
        "scala"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.scala", "**/*.sc"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Scala source: {}", path.display()))?;

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

        Ok(FileFacts {
            path: parsed.path.clone(),
            language: self.language_id().to_string(),
            package,
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

    fn parse_scala(source: &str) -> (ScalaAnalyzer, ParsedFile) {
        let analyzer = ScalaAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("Test.scala"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
package com.example

import scala.collection.mutable
import scala.util.Try

class Test
"#;
        let (analyzer, parsed) = parse_scala(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert_eq!(facts.package, Some("com.example".to_string()));
        assert!(facts.imports.iter().any(|i| i.path == "scala.collection.mutable"));
        assert!(facts.imports.iter().any(|i| i.path == "scala.util.Try"));
    }

    #[test]
    fn test_extract_declarations() {
        let source = r#"
class MyClass {
  def myMethod(): Unit = {}
}

object MyObject

trait MyTrait {
  def abstractMethod(): Int
}
"#;
        let (analyzer, parsed) = parse_scala(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "MyClass" && d.kind == DeclarationKind::Type));
        assert!(facts.declarations.iter().any(|d| d.name == "MyObject" && d.kind == DeclarationKind::Type));
        assert!(facts.declarations.iter().any(|d| d.name == "MyTrait" && d.kind == DeclarationKind::Interface));
    }
}

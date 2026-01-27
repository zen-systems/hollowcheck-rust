//! TypeScript language analyzer using tree-sitter.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

const DECLARATION_QUERY: &str = r#"
; Function declarations
(function_declaration
  name: (identifier) @func_name
) @function

; Function expressions assigned to variables
(variable_declarator
  name: (identifier) @var_func_name
  value: (function_expression)
) @var_function

; Arrow functions assigned to variables
(variable_declarator
  name: (identifier) @arrow_func_name
  value: (arrow_function)
) @arrow_function

; Method definitions in objects/classes
(method_definition
  name: (property_identifier) @method_name
) @method

; Class declarations
(class_declaration
  name: (type_identifier) @class_name
) @class

; Interface declarations
(interface_declaration
  name: (type_identifier) @interface_name
) @interface

; Type alias declarations
(type_alias_declaration
  name: (type_identifier) @type_alias_name
) @type_alias

; Enum declarations
(enum_declaration
  name: (identifier) @enum_name
) @enum
"#;

const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(for_in_statement) @for_in
(while_statement) @while
(do_statement) @do
(switch_statement) @switch
(switch_case) @case
(ternary_expression) @ternary
(binary_expression operator: "&&") @and
(binary_expression operator: "||") @or
(binary_expression operator: "??") @nullish
(try_statement) @try
(catch_clause) @catch
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
; import x from 'module'
(import_statement
  source: (string) @import_source
) @import

; import { x } from 'module'
(import_statement
  source: (string) @named_import_source
) @named_import

; import type { x } from 'module'
(import_statement
  source: (string) @type_import_source
) @type_import

; require('module')
(call_expression
  function: (identifier) @require_func (#eq? @require_func "require")
  arguments: (arguments (string) @require_source)
) @require

; export * from 'module'
(export_statement
  source: (string) @reexport_source
) @reexport
"#;

pub struct TypeScriptAnalyzer {
    language: Language,
}

impl TypeScriptAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
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
                    "func_name" | "var_func_name" | "arrow_func_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Function;
                    }
                    "method_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Method;
                    }
                    "class_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "interface_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Interface;
                    }
                    "type_alias_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "enum_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Enum;
                    }
                    "function" | "var_function" | "arrow_function" | "method" | "class"
                    | "interface" | "type_alias" | "enum" => {
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
        let body_node = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "statement_block");

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
            is_nil_return_only: self.is_null_return_only(parsed, body_node),
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

        statements[0].kind() == "throw_statement"
    }

    fn is_null_return_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        if stmt.kind() == "return_statement" {
            let text = parsed.node_text(stmt).trim();
            return text == "return null;"
                || text == "return null"
                || text == "return undefined;"
                || text == "return undefined";
        }
        false
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
                    "for" | "for_in" | "while" | "do" => info.loop_count += 1,
                    "switch" => info.switch_count += 1,
                    "case" => info.case_count += 1,
                    "ternary" => info.ternary_count += 1,
                    "and" | "or" | "nullish" => info.and_count += 1,
                    "catch" => info.catch_count += 1,
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
                    "import_source" | "named_import_source" | "type_import_source"
                    | "require_source" | "reexport_source" => {
                        // Remove quotes from path
                        let raw = parsed.node_text(capture.node);
                        path = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
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

impl Default for TypeScriptAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language_id(&self) -> &'static str {
        "typescript"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.ts", "**/*.tsx", "**/*.mts"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser.parse(source, None).ok_or_else(|| {
            anyhow::anyhow!("failed to parse TypeScript source: {}", path.display())
        })?;

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

    fn parse_ts(source: &str) -> (TypeScriptAnalyzer, ParsedFile) {
        let analyzer = TypeScriptAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("test.ts"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
import express from 'express';
import { Request, Response } from 'express';
import type { Handler } from './types';
import * as fs from 'fs';
"#;
        let (analyzer, parsed) = parse_ts(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.imports.iter().any(|i| i.path == "express"));
        assert!(facts.imports.iter().any(|i| i.path == "./types"));
        assert!(facts.imports.iter().any(|i| i.path == "fs"));
    }

    #[test]
    fn test_extract_declarations() {
        let source = r#"
function hello() {}

const greet = () => {};

class MyClass {
    method() {}
}

interface MyInterface {
    prop: string;
}

type MyType = string | number;

enum Status {
    Active,
    Inactive
}
"#;
        let (analyzer, parsed) = parse_ts(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "hello"));
        assert!(facts.declarations.iter().any(|d| d.name == "greet"));
        assert!(facts.declarations.iter().any(|d| d.name == "MyClass"));
        assert!(facts.declarations.iter().any(|d| d.name == "MyInterface"));
        assert!(facts.declarations.iter().any(|d| d.name == "MyType"));
        assert!(facts.declarations.iter().any(|d| d.name == "Status"));
    }

    #[test]
    fn test_stub_detection() {
        let source = r#"
function throwOnly() {
    throw new Error("not implemented");
}

function nullOnly(): null {
    return null;
}

function undefinedOnly(): undefined {
    return undefined;
}
"#;
        let (analyzer, parsed) = parse_ts(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let throw_only = facts.declarations.iter().find(|d| d.name == "throwOnly").unwrap();
        assert!(throw_only.body.as_ref().unwrap().is_panic_only);

        let null_only = facts.declarations.iter().find(|d| d.name == "nullOnly").unwrap();
        assert!(null_only.body.as_ref().unwrap().is_nil_return_only);
    }
}

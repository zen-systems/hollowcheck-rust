//! C language analyzer using tree-sitter.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

/// Tree-sitter query for extracting C declarations.
const DECLARATION_QUERY: &str = r#"
; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @func_name
  )
) @function

; Struct definitions
(struct_specifier
  name: (type_identifier) @struct_name
) @struct

; Enum definitions
(enum_specifier
  name: (type_identifier) @enum_name
) @enum

; Typedef definitions
(type_definition
  declarator: (type_identifier) @typedef_name
) @typedef
"#;

/// Tree-sitter query for control flow nodes.
const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(while_statement) @while
(do_statement) @do
(switch_statement) @switch
(case_statement) @case
(conditional_expression) @ternary
(binary_expression operator: "&&") @and
(binary_expression operator: "||") @or
"#;

/// Tree-sitter query for extracting includes.
const IMPORT_QUERY: &str = r#"
; #include <header.h>
(preproc_include
  path: (system_lib_string) @system_include
) @include_system

; #include "header.h"
(preproc_include
  path: (string_literal) @local_include
) @include_local
"#;

/// C language analyzer.
pub struct CAnalyzer {
    language: Language,
}

impl CAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_c::LANGUAGE.into(),
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
                        kind = DeclarationKind::Function;
                    }
                    "struct_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Struct;
                    }
                    "enum_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Enum;
                    }
                    "typedef_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "function" | "struct" | "enum" | "typedef" => {
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
        let body_node = func_node
            .children(&mut func_node.walk())
            .find(|n| n.kind() == "compound_statement");

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
            is_panic_only: false,
            is_nil_return_only: false,
            has_only_todo_comment: self.has_only_todo_comment(parsed, body_node),
            text: body_text,
            control_flow,
        }))
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
                    "for" | "while" | "do" => info.loop_count += 1,
                    "switch" => info.switch_count += 1,
                    "case" => info.case_count += 1,
                    "ternary" => info.ternary_count += 1,
                    "and" => info.and_count += 1,
                    "or" => info.or_count += 1,
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
                    "system_include" => {
                        // Remove angle brackets: <stdio.h> -> stdio.h
                        let raw = parsed.node_text(capture.node);
                        path = raw.trim_matches(|c| c == '<' || c == '>').to_string();
                        import_node = Some(capture.node);
                    }
                    "local_include" => {
                        // Remove quotes: "header.h" -> header.h
                        let raw = parsed.node_text(capture.node);
                        path = raw.trim_matches('"').to_string();
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

impl Default for CAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for CAnalyzer {
    fn language_id(&self) -> &'static str {
        "c"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.c", "**/*.h"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse C source: {}", path.display()))?;

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

    fn parse_c(source: &str) -> (CAnalyzer, ParsedFile) {
        let analyzer = CAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("test.c"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_includes() {
        let source = r#"
#include <stdio.h>
#include <stdlib.h>
#include "myheader.h"

int main() {
    return 0;
}
"#;
        let (analyzer, parsed) = parse_c(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.imports.iter().any(|i| i.path == "stdio.h"));
        assert!(facts.imports.iter().any(|i| i.path == "stdlib.h"));
        assert!(facts.imports.iter().any(|i| i.path == "myheader.h"));
    }

    #[test]
    fn test_extract_declarations() {
        let source = r#"
struct Point {
    int x;
    int y;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

int add(int a, int b) {
    return a + b;
}
"#;
        let (analyzer, parsed) = parse_c(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "Point" && d.kind == DeclarationKind::Struct));
        assert!(facts.declarations.iter().any(|d| d.name == "Color" && d.kind == DeclarationKind::Enum));
        assert!(facts.declarations.iter().any(|d| d.name == "add" && d.kind == DeclarationKind::Function));
    }
}

//! Swift language analyzer using tree-sitter.

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
  name: (simple_identifier) @func_name
) @function

; Class declarations
(class_declaration
  name: (type_identifier) @class_name
) @class
"#;

const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(while_statement) @while
(repeat_while_statement) @repeat
(switch_statement) @switch
(switch_entry) @case
(ternary_expression) @ternary
(conjunction_expression) @and
(disjunction_expression) @or
(do_statement) @do
(catch_block) @catch
(guard_statement) @guard
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
; import Foundation
(import_declaration
  (identifier) @import_module
) @import
"#;

pub struct SwiftAnalyzer {
    language: Language,
}

impl SwiftAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_swift::LANGUAGE.into(),
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
                    "class_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Type;
                    }
                    "function" | "class" => {
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
            .find(|n| n.kind() == "function_body");

        let body_node = match body_node {
            Some(n) => n,
            None => return Ok(None),
        };

        let body_text = parsed.node_text(body_node).to_string();
        let span = Span::from_node(body_node);

        let statement_count = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment" | "multiline_comment"))
            .count();

        let is_empty = statement_count == 0;
        let control_flow = self.extract_control_flow(parsed, body_node)?;

        Ok(Some(FunctionBody {
            span,
            statement_count,
            is_empty,
            is_panic_only: self.is_fatal_error_only(parsed, body_node),
            is_nil_return_only: self.is_nil_return_only(parsed, body_node),
            has_only_todo_comment: self.has_only_todo_comment(parsed, body_node),
            text: body_text,
            control_flow,
        }))
    }

    fn is_fatal_error_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment" | "multiline_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let text = parsed.node_text(statements[0]).trim();
        text.starts_with("fatalError(") || text.starts_with("preconditionFailure(")
    }

    fn is_nil_return_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "comment" | "multiline_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let text = parsed.node_text(statements[0]).trim();
        text == "return nil"
    }

    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "{" | "}" => continue,
                "comment" | "multiline_comment" => {
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
                    "if" | "guard" => info.if_count += 1,
                    "for" | "while" | "repeat" => info.loop_count += 1,
                    "switch" => info.switch_count += 1,
                    "case" => info.case_count += 1,
                    "ternary" => info.ternary_count += 1,
                    "and" => info.and_count += 1,
                    "or" => info.or_count += 1,
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
                if name == "import_module" {
                    path = parsed.node_text(capture.node).to_string();
                    import_node = Some(capture.node);
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

impl Default for SwiftAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SwiftAnalyzer {
    fn language_id(&self) -> &'static str {
        "swift"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.swift"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Swift source: {}", path.display()))?;

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

    fn parse_swift(source: &str) -> (SwiftAnalyzer, ParsedFile) {
        let analyzer = SwiftAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("Test.swift"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
import Foundation
import UIKit

class Test {}
"#;
        let (analyzer, parsed) = parse_swift(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.imports.iter().any(|i| i.path == "Foundation"));
        assert!(facts.imports.iter().any(|i| i.path == "UIKit"));
    }

    #[test]
    fn test_extract_declarations() {
        let source = r#"
class MyClass {
    func myMethod() {}
}

func topLevelFunc() {}
"#;
        let (analyzer, parsed) = parse_swift(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "MyClass" && d.kind == DeclarationKind::Type));
        assert!(facts.declarations.iter().any(|d| d.name == "topLevelFunc" && d.kind == DeclarationKind::Function));
    }

    #[test]
    fn test_stub_detection() {
        let source = r#"
func stubFatalError() {
    fatalError("not implemented")
}

func stubNil() -> String? {
    return nil
}
"#;
        let (analyzer, parsed) = parse_swift(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let fatal_error = facts.declarations.iter().find(|d| d.name == "stubFatalError").unwrap();
        assert!(fatal_error.body.as_ref().unwrap().is_panic_only);

        let stub_nil = facts.declarations.iter().find(|d| d.name == "stubNil").unwrap();
        assert!(stub_nil.body.as_ref().unwrap().is_nil_return_only);
    }
}

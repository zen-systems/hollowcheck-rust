//! Java language analyzer using tree-sitter.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::analysis::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import,
    LanguageAnalyzer, ParsedFile, Span,
};

const DECLARATION_QUERY: &str = r#"
; Method declarations
(method_declaration
  name: (identifier) @method_name
) @method

; Constructor declarations
(constructor_declaration
  name: (identifier) @constructor_name
) @constructor

; Class declarations
(class_declaration
  name: (identifier) @class_name
) @class

; Interface declarations
(interface_declaration
  name: (identifier) @interface_name
) @interface

; Enum declarations
(enum_declaration
  name: (identifier) @enum_name
) @enum
"#;

const CONTROL_FLOW_QUERY: &str = r#"
(if_statement) @if
(for_statement) @for
(enhanced_for_statement) @for_each
(while_statement) @while
(do_statement) @do
(switch_expression) @switch
(switch_block_statement_group) @case
(ternary_expression) @ternary
(try_statement) @try
(catch_clause) @catch
"#;

/// Tree-sitter query for extracting imports.
const IMPORT_QUERY: &str = r#"
; import com.package.Class;
(import_declaration
  (scoped_identifier) @import_path
) @import

; import static com.package.Class.method;
(import_declaration
  "static"
  (scoped_identifier) @static_import_path
) @static_import
"#;

/// Tree-sitter query for package declaration.
const PACKAGE_QUERY: &str = r#"
(package_declaration
  (scoped_identifier) @package_name
)
"#;

pub struct JavaAnalyzer {
    language: Language,
}

impl JavaAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_java::LANGUAGE.into(),
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
                    "method_name" | "constructor_name" => {
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
                    "enum_name" => {
                        name = parsed.node_text(capture.node).to_string();
                        kind = DeclarationKind::Enum;
                    }
                    "method" | "constructor" | "class" | "interface" | "enum" => {
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
            .find(|n| n.kind() == "block");

        let body_node = match body_node {
            Some(n) => n,
            None => return Ok(None),
        };

        let body_text = parsed.node_text(body_node).to_string();
        let span = Span::from_node(body_node);

        let statement_count = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .count();

        let is_empty = statement_count == 0;
        let is_panic_only = self.is_throw_only(parsed, body_node);
        let control_flow = self.extract_control_flow(parsed, body_node)?;

        Ok(Some(FunctionBody {
            span,
            statement_count,
            is_empty,
            is_panic_only,
            is_nil_return_only: self.is_null_return_only(parsed, body_node),
            has_only_todo_comment: self.has_only_todo_comment(parsed, body_node),
            text: body_text,
            control_flow,
        }))
    }

    fn is_throw_only(&self, _parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        stmt.kind() == "throw_statement"
    }

    fn is_null_return_only(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let statements: Vec<_> = body_node
            .children(&mut body_node.walk())
            .filter(|n| !matches!(n.kind(), "{" | "}" | "line_comment" | "block_comment"))
            .collect();

        if statements.len() != 1 {
            return false;
        }

        let stmt = statements[0];
        if stmt.kind() == "return_statement" {
            let text = parsed.node_text(stmt).trim();
            return text == "return null;" || text == "return null";
        }
        false
    }

    fn has_only_todo_comment(&self, parsed: &ParsedFile, body_node: tree_sitter::Node) -> bool {
        let mut has_todo = false;
        let mut has_other = false;

        for child in body_node.children(&mut body_node.walk()) {
            match child.kind() {
                "{" | "}" => continue,
                "line_comment" | "block_comment" => {
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
                    "for" | "for_each" | "while" | "do" => info.loop_count += 1,
                    "switch" => info.switch_count += 1,
                    "case" => info.case_count += 1,
                    "ternary" => info.ternary_count += 1,
                    "catch" => info.catch_count += 1,
                    _ => {}
                }
            }
        }

        Ok(info)
    }

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
                    "import_path" | "static_import_path" => {
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

impl Default for JavaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for JavaAnalyzer {
    fn language_id(&self) -> &'static str {
        "java"
    }

    fn file_globs(&self) -> &'static [&'static str] {
        &["**/*.java"]
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile> {
        let mut parser = self.create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse Java source: {}", path.display()))?;

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

    fn parse_java(source: &str) -> (JavaAnalyzer, ParsedFile) {
        let analyzer = JavaAnalyzer::new();
        let parsed = analyzer
            .parse(Path::new("Test.java"), source.as_bytes())
            .unwrap();
        (analyzer, parsed)
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
package com.example;

import java.util.List;
import java.util.Map;
import static java.lang.Math.PI;

public class Test {}
"#;
        let (analyzer, parsed) = parse_java(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert_eq!(facts.package, Some("com.example".to_string()));
        assert!(facts.imports.iter().any(|i| i.path == "java.util.List"));
        assert!(facts.imports.iter().any(|i| i.path == "java.util.Map"));
    }

    #[test]
    fn test_extract_classes() {
        let source = r#"
public class MyClass {
    public void myMethod() {}
}

interface MyInterface {
    void doSomething();
}

enum MyEnum {
    VALUE1, VALUE2
}
"#;
        let (analyzer, parsed) = parse_java(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        assert!(facts.declarations.iter().any(|d| d.name == "MyClass" && d.kind == DeclarationKind::Type));
        assert!(facts.declarations.iter().any(|d| d.name == "myMethod" && d.kind == DeclarationKind::Method));
        assert!(facts.declarations.iter().any(|d| d.name == "MyInterface" && d.kind == DeclarationKind::Interface));
        assert!(facts.declarations.iter().any(|d| d.name == "MyEnum" && d.kind == DeclarationKind::Enum));
    }

    #[test]
    fn test_stub_detection() {
        let source = r#"
public class Test {
    public void throwOnly() {
        throw new UnsupportedOperationException();
    }

    public Object nullOnly() {
        return null;
    }
}
"#;
        let (analyzer, parsed) = parse_java(source);
        let facts = analyzer.extract_facts(&parsed).unwrap();

        let throw_only = facts.declarations.iter().find(|d| d.name == "throwOnly").unwrap();
        assert!(throw_only.body.as_ref().unwrap().is_panic_only);

        let null_only = facts.declarations.iter().find(|d| d.name == "nullOnly").unwrap();
        assert!(null_only.body.as_ref().unwrap().is_nil_return_only);
    }
}

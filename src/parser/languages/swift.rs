//! Swift language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Swift symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `class_name`: Class, struct, enum, and actor names (all use class_declaration)
/// - `protocol_name`: Protocol names
const SYMBOL_QUERY: &str = r#"
(function_declaration name: (simple_identifier) @func_name) @function
(class_declaration name: (type_identifier) @class_name) @class
(protocol_declaration name: (type_identifier) @protocol_name) @protocol
"#;

/// Tree-sitter query for finding Swift functions by name.
const FUNCTION_QUERY: &str = r#"
(function_declaration name: (simple_identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Swift.
///
/// Counts:
/// - if statements
/// - guard statements
/// - switch statements
/// - switch entries (case/default)
/// - while statements
/// - for statements
/// - repeat-while statements
/// - do statements
/// - catch blocks
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(guard_statement) @branch
(switch_statement) @branch
(switch_entry) @branch
(while_statement) @branch
(for_statement) @branch
(repeat_while_statement) @branch
(do_statement) @branch
(catch_block) @branch
"#;

/// Symbol capture configurations for Swift.
/// Note: class_name captures class, struct, enum, and actor declarations
/// as they all use the same class_declaration node type in tree-sitter-swift.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "func_name",
        kind: "function",
    },
    SymbolCapture {
        name_capture: "class_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "protocol_name",
        kind: "type",
    },
];

/// Create a new Swift parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_swift::LANGUAGE.into(),
        language_name: "swift",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Swift parser for .swift extension.
pub fn register() {
    crate::parser::register(".swift", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swift_symbols() {
        let parser = new_parser();
        let source = br#"
import Foundation

class MyService {
    func processData(_ input: String) -> String {
        return input.uppercased()
    }
}

struct Config {
    var name: String
}

enum Status {
    case active
    case inactive
}

protocol Processor {
    func process()
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyService" && s.kind == "type"),
            "Expected MyService class, got: {:?}",
            symbols
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "processData" && s.kind == "function"),
            "Expected processData function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Config" && s.kind == "type"),
            "Expected Config struct"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Status" && s.kind == "type"),
            "Expected Status enum"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Processor" && s.kind == "type"),
            "Expected Processor protocol"
        );
    }

    #[test]
    fn test_swift_complexity_simple() {
        let parser = new_parser();
        let source = br#"
func simple() -> Int {
    return 42
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_swift_complexity_branches() {
        let parser = new_parser();
        let source = br#"
func branchy(x: Int) -> Int {
    if x > 0 {
        return 1
    } else if x < 0 {
        return -1
    } else {
        return 0
    }
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        assert!(complexity >= 2, "Expected >= 2, got {}", complexity);
    }

    #[test]
    fn test_swift_complexity_guard() {
        let parser = new_parser();
        let source = br#"
func guardy(x: Int?) -> Int {
    guard let value = x else {
        return 0
    }
    if value > 0 {
        return value
    }
    return -value
}
"#;

        let complexity = parser.complexity(source, "guardy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_swift_complexity_switch() {
        let parser = new_parser();
        let source = br#"
func switchy(x: String) -> Int {
    switch x {
    case "a":
        return 1
    case "b":
        return 2
    default:
        return 0
    }
}
"#;

        let complexity = parser.complexity(source, "switchy").unwrap();
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }
}

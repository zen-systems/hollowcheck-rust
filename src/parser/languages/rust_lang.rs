//! Rust language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Rust symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `method_name`: Method names (inside impl blocks)
/// - `struct_name`: Struct names
/// - `enum_name`: Enum names
/// - `trait_name`: Trait names
/// - `type_name`: Type alias names
/// - `const_name`: Constant names
const SYMBOL_QUERY: &str = r#"
(function_item name: (identifier) @func_name) @function
(impl_item (declaration_list (function_item name: (identifier) @method_name))) @method
(struct_item name: (type_identifier) @struct_name) @struct
(enum_item name: (type_identifier) @enum_name) @enum
(trait_item name: (type_identifier) @trait_name) @trait
(type_item name: (type_identifier) @type_name) @type
(const_item name: (identifier) @const_name) @const
"#;

/// Tree-sitter query for finding Rust functions by name.
const FUNCTION_QUERY: &str = r#"
(function_item name: (identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Rust.
///
/// Counts:
/// - if expressions
/// - else clauses
/// - for expressions
/// - while expressions
/// - loop expressions
/// - match expressions
/// - match arms
/// - logical operators (&&, ||)
const COMPLEXITY_QUERY: &str = r#"
(if_expression) @branch
(else_clause) @branch
(for_expression) @branch
(while_expression) @branch
(loop_expression) @branch
(match_expression) @branch
(match_arm) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
"#;

/// Symbol capture configurations for Rust.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "func_name",
        kind: "function",
    },
    SymbolCapture {
        name_capture: "method_name",
        kind: "method",
    },
    SymbolCapture {
        name_capture: "struct_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "enum_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "trait_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "type_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "const_name",
        kind: "const",
    },
];

/// Create a new Rust parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_rust::language(),
        language_name: "rust",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Rust parser for .rs extension.
pub fn register() {
    crate::parser::register(".rs", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_symbols() {
        let parser = new_parser();
        let source = br#"
const VERSION: &str = "1.0";

struct Config {
    name: String,
}

enum Status {
    Ok,
    Error,
}

trait Processor {
    fn process(&self);
}

type Result<T> = std::result::Result<T, Error>;

impl Config {
    fn new() -> Self {
        Config { name: String::new() }
    }
}

fn main() {
    println!("hello");
}

fn helper() -> i32 {
    42
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "VERSION" && s.kind == "const"),
            "Expected VERSION const, got: {:?}",
            symbols
        );
        assert!(
            symbols.iter().any(|s| s.name == "Config" && s.kind == "type"),
            "Expected Config struct"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Status" && s.kind == "type"),
            "Expected Status enum"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Processor" && s.kind == "type"),
            "Expected Processor trait"
        );
        assert!(
            symbols.iter().any(|s| s.name == "main" && s.kind == "function"),
            "Expected main function"
        );
        assert!(
            symbols.iter().any(|s| s.name == "helper" && s.kind == "function"),
            "Expected helper function"
        );
    }

    #[test]
    fn test_rust_complexity_simple() {
        let parser = new_parser();
        let source = br#"
fn simple() -> i32 {
    42
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_rust_complexity_branches() {
        let parser = new_parser();
        let source = br#"
fn branchy(x: i32) -> i32 {
    if x > 0 {
        1
    } else if x < 0 {
        -1
    } else {
        0
    }
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_rust_complexity_loops() {
        let parser = new_parser();
        let source = br#"
fn loopy(items: Vec<i32>) -> i32 {
    let mut sum = 0;
    for item in items {
        while item > 0 {
            sum += item;
        }
    }
    sum
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_rust_complexity_match() {
        let parser = new_parser();
        let source = br#"
fn matchy(x: Option<i32>) -> i32 {
    match x {
        Some(v) if v > 0 => v,
        Some(v) => -v,
        None => 0,
    }
}
"#;

        let complexity = parser.complexity(source, "matchy").unwrap();
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }

    #[test]
    fn test_rust_complexity_boolean_ops() {
        let parser = new_parser();
        let source = br#"
fn check(a: bool, b: bool, c: bool) -> bool {
    if a && b {
        true
    } else if a || c {
        true
    } else {
        false
    }
}
"#;

        let complexity = parser.complexity(source, "check").unwrap();
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_rust_complexity_missing_function() {
        let parser = new_parser();
        let source = br#"
fn existing() {
}
"#;

        let complexity = parser.complexity(source, "nonexistent").unwrap();
        assert_eq!(complexity, 0, "Missing function should return 0");
    }
}

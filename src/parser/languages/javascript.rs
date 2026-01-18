//! JavaScript language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding JavaScript symbols.
///
/// Captures:
/// - `func_name`: Function names from function declarations
/// - `method_name`: Method names from class methods
/// - `class_name`: Class names
const SYMBOL_QUERY: &str = r#"
(function_declaration name: (identifier) @func_name) @function
(method_definition name: (property_identifier) @method_name) @method
(class_declaration name: (identifier) @class_name) @class
"#;

/// Tree-sitter query for finding JavaScript functions by name.
const FUNCTION_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @func
(method_definition name: (property_identifier) @name) @func
(arrow_function) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in JavaScript.
///
/// Counts:
/// - if statements
/// - for loops (including for-in)
/// - while loops
/// - do-while loops
/// - switch statements
/// - switch cases
/// - catch clauses
/// - ternary expressions
/// - logical operators (&&, ||, ??)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(for_statement) @branch
(for_in_statement) @branch
(while_statement) @branch
(do_statement) @branch
(switch_statement) @branch
(switch_case) @branch
(catch_clause) @branch
(ternary_expression) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
(binary_expression operator: "??") @branch
"#;

/// Symbol capture configurations for JavaScript.
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
        name_capture: "class_name",
        kind: "type",
    },
];

/// Create a new JavaScript parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_javascript::LANGUAGE.into(),
        language_name: "javascript",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register JavaScript parser for .js and .jsx extensions.
pub fn register() {
    crate::parser::register(".js", new_parser);
    crate::parser::register(".jsx", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_javascript_symbols() {
        let parser = new_parser();
        let source = br#"
function hello() {
    console.log("hello");
}

function world(x, y) {
    return x + y;
}

class MyClass {
    constructor() {
        this.value = 0;
    }

    method() {
        return this.value;
    }
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "hello" && s.kind == "function"),
            "Expected hello function, got: {:?}",
            symbols
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "world" && s.kind == "function"),
            "Expected world function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyClass" && s.kind == "type"),
            "Expected MyClass"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "method" && s.kind == "method"),
            "Expected method"
        );
    }

    #[test]
    fn test_javascript_complexity_simple() {
        let parser = new_parser();
        let source = br#"
function simple() {
    return 42;
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_javascript_complexity_branches() {
        let parser = new_parser();
        let source = br#"
function branchy(x) {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return -1;
    } else {
        return 0;
    }
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        assert!(complexity >= 2, "Expected >= 2, got {}", complexity);
    }

    #[test]
    fn test_javascript_complexity_loops() {
        let parser = new_parser();
        let source = br#"
function loopy(items) {
    let result = [];
    for (let i = 0; i < items.length; i++) {
        while (items[i] > 0) {
            result.push(items[i]);
            items[i]--;
        }
    }
    return result;
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_javascript_complexity_nullish() {
        let parser = new_parser();
        let source = br#"
function check(a, b) {
    const x = a ?? b;
    if (a && b) {
        return true;
    }
    if (a || b) {
        return true;
    }
    return false;
}
"#;

        let complexity = parser.complexity(source, "check").unwrap();
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_javascript_complexity_missing_function() {
        let parser = new_parser();
        let source = br#"
function existing() {
}
"#;

        let complexity = parser.complexity(source, "nonexistent").unwrap();
        assert_eq!(complexity, 0, "Missing function should return 0");
    }
}

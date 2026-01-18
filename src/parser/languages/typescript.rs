//! TypeScript/JavaScript language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding TypeScript symbols.
///
/// Captures:
/// - `func_name`: Function names from function_declaration
/// - `method_name`: Method names from method_definition
/// - `class_name`: Class names from class_declaration
/// - `interface_name`: Interface names (TypeScript)
/// - `type_name`: Type alias names (TypeScript)
const SYMBOL_QUERY: &str = r#"
(function_declaration name: (identifier) @func_name) @function
(method_definition name: (property_identifier) @method_name) @method
(class_declaration name: (type_identifier) @class_name) @class
(interface_declaration name: (type_identifier) @interface_name) @interface
(type_alias_declaration name: (type_identifier) @type_name) @type
"#;

/// Tree-sitter query for finding functions by name.
const FUNCTION_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @func
(method_definition name: (property_identifier) @name) @func
(arrow_function) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in TypeScript/JavaScript.
///
/// Counts:
/// - if statements
/// - for loops (regular, for-in, for-of)
/// - while/do-while loops
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

/// Symbol capture configurations for TypeScript.
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
    SymbolCapture {
        name_capture: "interface_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "type_name",
        kind: "type",
    },
];

/// Create a new TypeScript parser.
pub fn new_typescript_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_typescript::language_typescript().into(),
        language_name: "typescript",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Create a new JavaScript parser (uses TypeScript parser with JSX support).
pub fn new_javascript_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_typescript::language_tsx().into(),
        language_name: "javascript",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register TypeScript and JavaScript parsers.
pub fn register() {
    crate::parser::register(".ts", new_typescript_parser);
    crate::parser::register(".tsx", new_typescript_parser);
    crate::parser::register(".js", new_javascript_parser);
    crate::parser::register(".jsx", new_javascript_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_symbols() {
        let parser = new_typescript_parser();
        let source = br#"
function hello(): void {
    console.log("hello");
}

class MyClass {
    method(): number {
        return 42;
    }
}

interface IConfig {
    name: string;
}

type StringArray = string[];
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "hello" && s.kind == "function"),
            "Expected hello function"
        );
        assert!(
            symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
            "Expected MyClass"
        );
        assert!(
            symbols.iter().any(|s| s.name == "method" && s.kind == "method"),
            "Expected method"
        );
        assert!(
            symbols.iter().any(|s| s.name == "IConfig" && s.kind == "type"),
            "Expected IConfig interface"
        );
        assert!(
            symbols.iter().any(|s| s.name == "StringArray" && s.kind == "type"),
            "Expected StringArray type"
        );
    }

    #[test]
    fn test_typescript_complexity_simple() {
        let parser = new_typescript_parser();
        let source = br#"
function simple(): number {
    return 42;
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_typescript_complexity_branches() {
        let parser = new_typescript_parser();
        let source = br#"
function branchy(x: number): number {
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
        // 1 (base) + 2 (if statements)
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_typescript_complexity_loops() {
        let parser = new_typescript_parser();
        let source = br#"
function loopy(items: number[]): number {
    let sum = 0;
    for (const item of items) {
        while (item > 0) {
            sum += item;
        }
    }
    return sum;
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        // 1 (base) + 1 (for-in) + 1 (while) = 3
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_typescript_complexity_switch() {
        let parser = new_typescript_parser();
        let source = br#"
function switchy(x: string): number {
    switch (x) {
        case "a":
            return 1;
        case "b":
            return 2;
        case "c":
            return 3;
        default:
            return 0;
    }
}
"#;

        let complexity = parser.complexity(source, "switchy").unwrap();
        // 1 (base) + 1 (switch) + 4 (cases including default)
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_typescript_complexity_ternary_and_nullish() {
        let parser = new_typescript_parser();
        let source = br#"
function ternary(x: number | null): number {
    const y = x ?? 0;
    return x > 0 ? x : -x;
}
"#;

        let complexity = parser.complexity(source, "ternary").unwrap();
        // 1 (base) + 1 (??) + 1 (ternary) = 3
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_javascript_symbols() {
        let parser = new_javascript_parser();
        let source = br#"
function hello() {
    console.log("hello");
}

class MyClass {
    method() {
        return 42;
    }
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "hello" && s.kind == "function"),
            "Expected hello function"
        );
        assert!(
            symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
            "Expected MyClass"
        );
    }
}

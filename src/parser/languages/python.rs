//! Python language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Python symbols.
///
/// Captures:
/// - `func_name`: Function names from function_definition
/// - `class_name`: Class names from class_definition
const SYMBOL_QUERY: &str = r#"
(function_definition name: (identifier) @func_name) @function
(class_definition name: (identifier) @class_name) @class
"#;

/// Tree-sitter query for finding Python functions by name.
const FUNCTION_QUERY: &str = r#"
(function_definition name: (identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Python.
///
/// Counts:
/// - if statements
/// - elif clauses
/// - for loops
/// - while loops
/// - except clauses
/// - with statements
/// - conditional expressions (ternary)
/// - boolean operators (and, or)
/// - comprehensions (list, dict, set, generator)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(elif_clause) @branch
(for_statement) @branch
(while_statement) @branch
(except_clause) @branch
(with_statement) @branch
(conditional_expression) @branch
(boolean_operator operator: "and") @branch
(boolean_operator operator: "or") @branch
(list_comprehension) @branch
(dictionary_comprehension) @branch
(set_comprehension) @branch
(generator_expression) @branch
"#;

/// Symbol capture configurations for Python.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "func_name",
        kind: "function",
    },
    SymbolCapture {
        name_capture: "class_name",
        kind: "type",
    },
];

/// Create a new Python parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_python::LANGUAGE.into(),
        language_name: "python",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Python parser for .py extension.
pub fn register() {
    crate::parser::register(".py", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_symbols() {
        let parser = new_parser();
        let source = br#"
def hello():
    pass

def world(x, y):
    return x + y

class MyClass:
    def __init__(self):
        pass

    def method(self):
        pass

class AnotherClass:
    pass
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        // Should find functions
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "hello" && s.kind == "function"),
            "Expected hello function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "world" && s.kind == "function"),
            "Expected world function"
        );

        // Should find classes
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyClass" && s.kind == "type"),
            "Expected MyClass"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "AnotherClass" && s.kind == "type"),
            "Expected AnotherClass"
        );
    }

    #[test]
    fn test_python_complexity_simple() {
        let parser = new_parser();
        let source = br#"
def simple():
    return 42
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_python_complexity_branches() {
        let parser = new_parser();
        let source = br#"
def branchy(x):
    if x > 0:
        return 1
    elif x < 0:
        return -1
    else:
        return 0
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        // 1 (base) + 1 (if) + 1 (elif) = 3
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_python_complexity_loops() {
        let parser = new_parser();
        let source = br#"
def loopy(items):
    result = []
    for item in items:
        while item > 0:
            result.append(item)
            item -= 1
    return result
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        // 1 (base) + 1 (for) + 1 (while) = 3
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_python_complexity_boolean_ops() {
        let parser = new_parser();
        let source = br#"
def check(a, b, c):
    if a and b:
        return True
    if a or c:
        return True
    return False
"#;

        let complexity = parser.complexity(source, "check").unwrap();
        // 1 (base) + 2 (if) + 1 (and) + 1 (or) = 5
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_python_complexity_comprehension() {
        let parser = new_parser();
        let source = br#"
def comprehend(items):
    return [x * 2 for x in items if x > 0]
"#;

        let complexity = parser.complexity(source, "comprehend").unwrap();
        // 1 (base) + 1 (list_comprehension) = 2
        // Note: the `if` inside comprehension might also count
        assert!(complexity >= 2, "Expected >= 2, got {}", complexity);
    }

    #[test]
    fn test_python_complexity_missing_function() {
        let parser = new_parser();
        let source = br#"
def existing():
    pass
"#;

        let complexity = parser.complexity(source, "nonexistent").unwrap();
        assert_eq!(complexity, 0, "Missing function should return 0");
    }
}

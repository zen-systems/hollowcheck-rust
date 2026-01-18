//! C language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding C symbols.
///
/// Captures:
/// - `func_name`: Function names from function definitions
/// - `func_decl_name`: Function names from declarations
/// - `struct_name`: Struct names
/// - `enum_name`: Enum names
/// - `typedef_name`: Typedef names
const SYMBOL_QUERY: &str = r#"
(function_definition declarator: (function_declarator declarator: (identifier) @func_name)) @function
(declaration declarator: (function_declarator declarator: (identifier) @func_decl_name)) @function_decl
(struct_specifier name: (type_identifier) @struct_name) @struct
(enum_specifier name: (type_identifier) @enum_name) @enum
(type_definition declarator: (type_identifier) @typedef_name) @typedef
"#;

/// Tree-sitter query for finding C functions by name.
const FUNCTION_QUERY: &str = r#"
(function_definition declarator: (function_declarator declarator: (identifier) @name)) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in C.
///
/// Counts:
/// - if statements
/// - else clauses
/// - for loops
/// - while loops
/// - do-while loops
/// - switch statements
/// - case statements
/// - conditional expressions (ternary)
/// - logical operators (&&, ||)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(else_clause) @branch
(for_statement) @branch
(while_statement) @branch
(do_statement) @branch
(switch_statement) @branch
(case_statement) @branch
(conditional_expression) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
"#;

/// Symbol capture configurations for C.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "func_name",
        kind: "function",
    },
    SymbolCapture {
        name_capture: "func_decl_name",
        kind: "function",
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
        name_capture: "typedef_name",
        kind: "type",
    },
];

/// Create a new C parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_c::LANGUAGE.into(),
        language_name: "c",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register C parser for .c and .h extensions.
pub fn register() {
    crate::parser::register(".c", new_parser);
    crate::parser::register(".h", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_symbols() {
        let parser = new_parser();
        let source = br#"
typedef int MyInt;

struct Config {
    int value;
};

enum Status {
    OK,
    ERROR
};

int process_data(const char* input) {
    return 0;
}

void helper(void) {
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyInt" && s.kind == "type"),
            "Expected MyInt typedef, got: {:?}",
            symbols
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
                .any(|s| s.name == "process_data" && s.kind == "function"),
            "Expected process_data function"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "helper" && s.kind == "function"),
            "Expected helper function"
        );
    }

    #[test]
    fn test_c_complexity_simple() {
        let parser = new_parser();
        let source = br#"
int simple(void) {
    return 42;
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_c_complexity_branches() {
        let parser = new_parser();
        let source = br#"
int branchy(int x) {
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
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_c_complexity_loops() {
        let parser = new_parser();
        let source = br#"
int loopy(int n) {
    int sum = 0;
    for (int i = 0; i < n; i++) {
        while (i > 0) {
            sum += i;
            i--;
        }
    }
    return sum;
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_c_complexity_switch() {
        let parser = new_parser();
        let source = br#"
int switchy(int x) {
    switch (x) {
    case 1:
        return 1;
    case 2:
        return 2;
    default:
        return 0;
    }
}
"#;

        let complexity = parser.complexity(source, "switchy").unwrap();
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }
}

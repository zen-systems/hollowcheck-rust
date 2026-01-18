//! C++ language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding C++ symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `method_name`: Method names (qualified identifiers)
/// - `class_name`: Class names
/// - `struct_name`: Struct names
/// - `enum_name`: Enum names
/// - `typedef_name`: Typedef names
const SYMBOL_QUERY: &str = r#"
(function_definition declarator: (function_declarator declarator: (identifier) @func_name)) @function
(function_definition declarator: (function_declarator declarator: (qualified_identifier name: (identifier) @method_name))) @method
(class_specifier name: (type_identifier) @class_name) @class
(struct_specifier name: (type_identifier) @struct_name) @struct
(enum_specifier name: (type_identifier) @enum_name) @enum
(type_definition declarator: (type_identifier) @typedef_name) @typedef
"#;

/// Tree-sitter query for finding C++ functions by name.
const FUNCTION_QUERY: &str = r#"
(function_definition declarator: (function_declarator declarator: (identifier) @name)) @func
(function_definition declarator: (function_declarator declarator: (qualified_identifier name: (identifier) @name))) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in C++.
///
/// Counts:
/// - if statements
/// - else clauses
/// - for loops (including range-based)
/// - while loops
/// - do-while loops
/// - switch statements
/// - case statements
/// - catch clauses
/// - conditional expressions (ternary)
/// - logical operators (&&, ||)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(else_clause) @branch
(for_statement) @branch
(for_range_loop) @branch
(while_statement) @branch
(do_statement) @branch
(switch_statement) @branch
(case_statement) @branch
(catch_clause) @branch
(conditional_expression) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
"#;

/// Symbol capture configurations for C++.
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

/// Create a new C++ parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_cpp::language(),
        language_name: "cpp",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register C++ parser for .cpp, .cc, .cxx, .hpp extensions.
pub fn register() {
    crate::parser::register(".cpp", new_parser);
    crate::parser::register(".cc", new_parser);
    crate::parser::register(".cxx", new_parser);
    crate::parser::register(".hpp", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpp_symbols() {
        let parser = new_parser();
        let source = br#"
typedef int MyInt;

class Config {
public:
    int value;
    void process();
};

struct Data {
    int x;
};

enum Status {
    OK,
    ERROR
};

int process_data(const std::string& input) {
    return 0;
}

void helper() {
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "MyInt" && s.kind == "type"),
            "Expected MyInt typedef, got: {:?}",
            symbols
        );
        assert!(
            symbols.iter().any(|s| s.name == "Config" && s.kind == "type"),
            "Expected Config class"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Data" && s.kind == "type"),
            "Expected Data struct"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Status" && s.kind == "type"),
            "Expected Status enum"
        );
        assert!(
            symbols.iter().any(|s| s.name == "process_data" && s.kind == "function"),
            "Expected process_data function"
        );
    }

    #[test]
    fn test_cpp_complexity_simple() {
        let parser = new_parser();
        let source = br#"
int simple() {
    return 42;
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_cpp_complexity_branches() {
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
    fn test_cpp_complexity_range_loop() {
        let parser = new_parser();
        let source = br#"
#include <vector>

int sum_items(const std::vector<int>& items) {
    int sum = 0;
    for (const auto& item : items) {
        if (item > 0) {
            sum += item;
        }
    }
    return sum;
}
"#;

        let complexity = parser.complexity(source, "sum_items").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_cpp_complexity_try_catch() {
        let parser = new_parser();
        let source = br#"
int risky(int x) {
    try {
        if (x < 0) {
            throw "error";
        }
        return x;
    } catch (...) {
        return 0;
    }
}
"#;

        let complexity = parser.complexity(source, "risky").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }
}

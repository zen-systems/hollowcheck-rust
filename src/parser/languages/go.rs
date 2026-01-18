//! Go language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Go symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `method_name`: Method names (functions with receivers)
/// - `type_name`: Type names
/// - `const_name`: Constant names
const SYMBOL_QUERY: &str = r#"
(function_declaration name: (identifier) @func_name) @function
(method_declaration name: (field_identifier) @method_name) @method
(type_declaration (type_spec name: (type_identifier) @type_name)) @type
(const_declaration (const_spec name: (identifier) @const_name)) @const
"#;

/// Tree-sitter query for finding Go functions by name.
const FUNCTION_QUERY: &str = r#"
(function_declaration name: (identifier) @name) @func
(method_declaration name: (field_identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Go.
///
/// Counts:
/// - if statements
/// - for loops (including range)
/// - switch statements (expression and type)
/// - select statements
/// - case clauses (switch and select)
/// - logical operators (&&, ||)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(for_statement) @branch
(expression_switch_statement) @branch
(type_switch_statement) @branch
(select_statement) @branch
(communication_case) @branch
(expression_case) @branch
(type_case) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
"#;

/// Symbol capture configurations for Go.
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
        name_capture: "type_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "const_name",
        kind: "const",
    },
];

/// Create a new Go parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_go::language(),
        language_name: "go",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Go parser for .go extension.
pub fn register() {
    crate::parser::register(".go", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_symbols() {
        let parser = new_parser();
        let source = br#"
package main

const Version = "1.0"

type Config struct {
    Name string
}

func (c *Config) Validate() error {
    return nil
}

func main() {
    fmt.Println("hello")
}

func helper() int {
    return 42
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "Version" && s.kind == "const"),
            "Expected Version const, got: {:?}",
            symbols
        );
        assert!(
            symbols.iter().any(|s| s.name == "Config" && s.kind == "type"),
            "Expected Config type"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Validate" && s.kind == "method"),
            "Expected Validate method"
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
    fn test_go_complexity_simple() {
        let parser = new_parser();
        let source = br#"
package main

func simple() int {
    return 42
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_go_complexity_branches() {
        let parser = new_parser();
        let source = br#"
package main

func branchy(x int) int {
    if x > 0 {
        return 1
    } else if x < 0 {
        return -1
    }
    return 0
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        // 1 (base) + 2 (if statements)
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_go_complexity_loops() {
        let parser = new_parser();
        let source = br#"
package main

func loopy(items []int) int {
    sum := 0
    for _, item := range items {
        for item > 0 {
            sum += item
            item--
        }
    }
    return sum
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        // 1 (base) + 2 (for loops)
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_go_complexity_switch() {
        let parser = new_parser();
        let source = br#"
package main

func switchy(x string) int {
    switch x {
    case "a":
        return 1
    case "b":
        return 2
    case "c":
        return 3
    default:
        return 0
    }
}
"#;

        let complexity = parser.complexity(source, "switchy").unwrap();
        // 1 (base) + 1 (switch) + 4 (cases)
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_go_complexity_select() {
        let parser = new_parser();
        let source = br#"
package main

func selecty(ch1, ch2 chan int) int {
    select {
    case x := <-ch1:
        return x
    case y := <-ch2:
        return y
    default:
        return 0
    }
}
"#;

        let complexity = parser.complexity(source, "selecty").unwrap();
        // 1 (base) + 1 (select) + 3 (cases)
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }

    #[test]
    fn test_go_complexity_boolean_ops() {
        let parser = new_parser();
        let source = br#"
package main

func check(a, b, c int) bool {
    if a > 0 && b > 0 {
        return true
    }
    if a < 0 || c < 0 {
        return false
    }
    return true
}
"#;

        let complexity = parser.complexity(source, "check").unwrap();
        // 1 (base) + 2 (if) + 1 (&&) + 1 (||) = 5
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_go_complexity_missing_function() {
        let parser = new_parser();
        let source = br#"
package main

func existing() {
}
"#;

        let complexity = parser.complexity(source, "nonexistent").unwrap();
        assert_eq!(complexity, 0, "Missing function should return 0");
    }
}

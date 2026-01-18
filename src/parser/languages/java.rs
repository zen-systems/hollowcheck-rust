//! Java language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Java symbols.
///
/// Captures:
/// - `class_name`: Class names
/// - `interface_name`: Interface names
/// - `enum_name`: Enum names
/// - `method_name`: Method names
const SYMBOL_QUERY: &str = r#"
(class_declaration name: (identifier) @class_name) @class
(interface_declaration name: (identifier) @interface_name) @interface
(enum_declaration name: (identifier) @enum_name) @enum
(method_declaration name: (identifier) @method_name) @method
"#;

/// Tree-sitter query for finding Java methods by name.
const FUNCTION_QUERY: &str = r#"
(method_declaration name: (identifier) @name) @func
(constructor_declaration name: (identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Java.
///
/// Counts:
/// - if statements
/// - for loops (regular and enhanced)
/// - while/do-while loops
/// - switch cases
/// - catch clauses
/// - ternary expressions
/// - logical operators (&&, ||)
const COMPLEXITY_QUERY: &str = r#"
(if_statement) @branch
(for_statement) @branch
(enhanced_for_statement) @branch
(while_statement) @branch
(do_statement) @branch
(switch_expression) @branch
(switch_block_statement_group) @branch
(catch_clause) @branch
(ternary_expression) @branch
(binary_expression operator: "&&") @branch
(binary_expression operator: "||") @branch
"#;

/// Symbol capture configurations for Java.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "class_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "interface_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "enum_name",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "method_name",
        kind: "method",
    },
];

/// Create a new Java parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_java::LANGUAGE.into(),
        language_name: "java",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Java parser for .java extension.
pub fn register() {
    crate::parser::register(".java", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_symbols() {
        let parser = new_parser();
        let source = br#"
public class MyClass {
    public void method() {
        System.out.println("hello");
    }

    public int calculate(int x) {
        return x * 2;
    }
}

interface MyInterface {
    void doSomething();
}

enum Status {
    ACTIVE,
    INACTIVE
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

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
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "calculate" && s.kind == "method"),
            "Expected calculate"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyInterface" && s.kind == "type"),
            "Expected MyInterface"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Status" && s.kind == "type"),
            "Expected Status enum"
        );
    }

    #[test]
    fn test_java_complexity_simple() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public int simple() {
        return 42;
    }
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple method should have complexity 1");
    }

    #[test]
    fn test_java_complexity_branches() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public int branchy(int x) {
        if (x > 0) {
            return 1;
        } else if (x < 0) {
            return -1;
        } else {
            return 0;
        }
    }
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        // 1 (base) + 2 (if statements)
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_java_complexity_loops() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public int loopy(int[] items) {
        int sum = 0;
        for (int item : items) {
            while (item > 0) {
                sum += item;
                item--;
            }
        }
        return sum;
    }
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        // 1 (base) + 1 (enhanced for) + 1 (while) = 3
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_java_complexity_switch() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public int switchy(String x) {
        switch (x) {
            case "a":
                return 1;
            case "b":
                return 2;
            default:
                return 0;
        }
    }
}
"#;

        let complexity = parser.complexity(source, "switchy").unwrap();
        // 1 (base) + 3 (cases)
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }

    #[test]
    fn test_java_complexity_try_catch() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public void risky() {
        try {
            throw new Exception();
        } catch (RuntimeException e) {
            System.out.println("runtime");
        } catch (Exception e) {
            System.out.println("exception");
        }
    }
}
"#;

        let complexity = parser.complexity(source, "risky").unwrap();
        // 1 (base) + 2 (catch clauses)
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_java_complexity_boolean_ops() {
        let parser = new_parser();
        let source = br#"
public class Test {
    public boolean check(int a, int b, int c) {
        if (a > 0 && b > 0) {
            return true;
        }
        if (a < 0 || c < 0) {
            return false;
        }
        return true;
    }
}
"#;

        let complexity = parser.complexity(source, "check").unwrap();
        // 1 (base) + 2 (if) + 1 (&&) + 1 (||) = 5
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }
}

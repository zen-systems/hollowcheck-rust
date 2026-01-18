//! Kotlin language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Kotlin symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `class_name`: Class names
/// - `object_name`: Object declaration names
const SYMBOL_QUERY: &str = r#"
(function_declaration (simple_identifier) @func_name) @function
(class_declaration (type_identifier) @class_name) @class
(object_declaration (type_identifier) @object_name) @object
"#;

/// Tree-sitter query for finding Kotlin functions by name.
const FUNCTION_QUERY: &str = r#"
(function_declaration (simple_identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Kotlin.
///
/// Counts:
/// - if expressions
/// - when expressions
/// - when entries
/// - for loops
/// - while loops
/// - do-while loops
/// - catch blocks
/// - conjunction expressions (&&)
/// - disjunction expressions (||)
/// - elvis expressions (?:)
const COMPLEXITY_QUERY: &str = r#"
(if_expression) @branch
(when_expression) @branch
(when_entry) @branch
(for_statement) @branch
(while_statement) @branch
(do_while_statement) @branch
(catch_block) @branch
(conjunction_expression) @branch
(disjunction_expression) @branch
(elvis_expression) @branch
"#;

/// Symbol capture configurations for Kotlin.
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
        name_capture: "object_name",
        kind: "type",
    },
];

/// Create a new Kotlin parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_kotlin::language(),
        language_name: "kotlin",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Kotlin parser for .kt and .kts extensions.
pub fn register() {
    crate::parser::register(".kt", new_parser);
    crate::parser::register(".kts", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kotlin_symbols() {
        let parser = new_parser();
        let source = br#"
fun hello() {
    println("hello")
}

fun world(x: Int, y: Int): Int {
    return x + y
}

class MyClass {
    fun method(): Int {
        return 42
    }
}

object Singleton {
    val value = 1
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols.iter().any(|s| s.name == "hello" && s.kind == "function"),
            "Expected hello function, got: {:?}",
            symbols
        );
        assert!(
            symbols.iter().any(|s| s.name == "world" && s.kind == "function"),
            "Expected world function"
        );
        assert!(
            symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
            "Expected MyClass"
        );
        assert!(
            symbols.iter().any(|s| s.name == "Singleton" && s.kind == "type"),
            "Expected Singleton object"
        );
    }

    #[test]
    fn test_kotlin_complexity_simple() {
        let parser = new_parser();
        let source = br#"
fun simple(): Int {
    return 42
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_kotlin_complexity_branches() {
        let parser = new_parser();
        let source = br#"
fun branchy(x: Int): Int {
    return if (x > 0) {
        1
    } else if (x < 0) {
        -1
    } else {
        0
    }
}
"#;

        let complexity = parser.complexity(source, "branchy").unwrap();
        assert!(complexity >= 2, "Expected >= 2, got {}", complexity);
    }

    #[test]
    fn test_kotlin_complexity_when() {
        let parser = new_parser();
        let source = br#"
fun wheny(x: String): Int {
    return when (x) {
        "a" -> 1
        "b" -> 2
        "c" -> 3
        else -> 0
    }
}
"#;

        let complexity = parser.complexity(source, "wheny").unwrap();
        assert!(complexity >= 5, "Expected >= 5, got {}", complexity);
    }

    #[test]
    fn test_kotlin_complexity_loops() {
        let parser = new_parser();
        let source = br#"
fun loopy(items: List<Int>): Int {
    var sum = 0
    for (item in items) {
        while (item > 0) {
            sum += item
        }
    }
    return sum
}
"#;

        let complexity = parser.complexity(source, "loopy").unwrap();
        assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
    }

    #[test]
    fn test_kotlin_complexity_elvis() {
        let parser = new_parser();
        let source = br#"
fun elvisey(x: String?, y: String?): String {
    val a = x ?: "default"
    val b = y ?: "other"
    return if (a.isNotEmpty() && b.isNotEmpty()) {
        a + b
    } else {
        a
    }
}
"#;

        let complexity = parser.complexity(source, "elvisey").unwrap();
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }

    #[test]
    fn test_kotlin_complexity_missing_function() {
        let parser = new_parser();
        let source = br#"
fun existing() {
}
"#;

        let complexity = parser.complexity(source, "nonexistent").unwrap();
        assert_eq!(complexity, 0, "Missing function should return 0");
    }
}

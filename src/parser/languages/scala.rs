//! Scala language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding Scala symbols.
///
/// Captures:
/// - `func_name`: Function names
/// - `class_name`: Class names
/// - `object_name`: Object (singleton) names
/// - `trait_name`: Trait names
const SYMBOL_QUERY: &str = r#"
(function_definition name: (identifier) @func_name) @function
(class_definition name: (identifier) @class_name) @class
(object_definition name: (identifier) @object_name) @object
(trait_definition name: (identifier) @trait_name) @trait
"#;

/// Tree-sitter query for finding Scala functions by name.
const FUNCTION_QUERY: &str = r#"
(function_definition name: (identifier) @name) @func
"#;

/// Tree-sitter query for counting cyclomatic complexity in Scala.
///
/// Counts:
/// - if expressions
/// - match expressions
/// - case clauses
/// - while expressions
/// - for expressions
/// - try expressions
/// - catch clauses
const COMPLEXITY_QUERY: &str = r#"
(if_expression) @branch
(match_expression) @branch
(case_clause) @branch
(while_expression) @branch
(for_expression) @branch
(try_expression) @branch
(catch_clause) @branch
"#;

/// Symbol capture configurations for Scala.
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
    SymbolCapture {
        name_capture: "trait_name",
        kind: "type",
    },
];

/// Create a new Scala parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_scala::LANGUAGE.into(),
        language_name: "scala",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "func",
        func_name_capture: "name",
    }))
}

/// Register Scala parser for .scala and .sc extensions.
pub fn register() {
    crate::parser::register(".scala", new_parser);
    crate::parser::register(".sc", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scala_symbols() {
        let parser = new_parser();
        let source = br#"
package com.example

object MyService {
  def processData(input: String): String = {
    input.toUpperCase
  }
}

class Config(val name: String)

trait Processor {
  def process(): Unit
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        assert!(
            symbols
                .iter()
                .any(|s| s.name == "MyService" && s.kind == "type"),
            "Expected MyService object, got: {:?}",
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
            "Expected Config class"
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Processor" && s.kind == "type"),
            "Expected Processor trait"
        );
    }

    #[test]
    fn test_scala_complexity_simple() {
        let parser = new_parser();
        let source = br#"
def simple(): Int = {
  42
}
"#;

        let complexity = parser.complexity(source, "simple").unwrap();
        assert_eq!(complexity, 1, "Simple function should have complexity 1");
    }

    #[test]
    fn test_scala_complexity_branches() {
        let parser = new_parser();
        let source = br#"
def branchy(x: Int): Int = {
  if (x > 0) {
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
    fn test_scala_complexity_match() {
        let parser = new_parser();
        let source = br#"
def matchy(x: String): Int = x match {
  case "a" => 1
  case "b" => 2
  case _ => 0
}
"#;

        let complexity = parser.complexity(source, "matchy").unwrap();
        assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
    }
}

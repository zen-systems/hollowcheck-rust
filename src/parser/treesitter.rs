//! Tree-sitter based parser implementation.
//!
//! This module provides a generic tree-sitter parser that can be configured
//! for different languages via queries.

#[cfg(feature = "tree-sitter")]
use streaming_iterator::StreamingIterator;
#[cfg(feature = "tree-sitter")]
use tree_sitter::{Language, Parser as TsParser, Query, QueryCursor};

use super::{Parser, Symbol, SymbolWithComplexity};

/// Defines how to extract symbol info from query captures.
#[derive(Debug, Clone)]
pub struct SymbolCapture {
    /// Capture name for the symbol name (e.g., "func_name")
    pub name_capture: &'static str,
    /// Symbol kind (e.g., "function", "type")
    pub kind: &'static str,
}

/// Configuration for a tree-sitter language parser.
#[derive(Clone)]
pub struct Config {
    /// The tree-sitter language
    pub language: Language,
    /// Language name (e.g., "python", "go")
    pub language_name: &'static str,
    /// Tree-sitter query for finding symbols
    pub symbol_query: &'static str,
    /// How to map captures to symbols
    pub symbol_captures: &'static [SymbolCapture],
    /// Query for counting complexity branch points
    pub complexity_query: &'static str,
    /// Query for finding function nodes by name
    pub function_query: &'static str,
    /// Capture name for function node in function_query
    pub function_capture: &'static str,
    /// Capture name for function name within function_query
    pub func_name_capture: &'static str,
}

/// Tree-sitter based parser.
pub struct TreeSitterParser {
    config: Config,
}

impl TreeSitterParser {
    /// Create a new tree-sitter parser with the given configuration.
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Parse source code and return the tree.
    fn parse(&self, source: &[u8]) -> anyhow::Result<tree_sitter::Tree> {
        let mut parser = TsParser::new();
        parser.set_language(&self.config.language)?;
        parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse source"))
    }

    /// Find a function node by name.
    fn find_function<'a>(
        &self,
        root: tree_sitter::Node<'a>,
        source: &[u8],
        name: &str,
    ) -> anyhow::Result<Option<tree_sitter::Node<'a>>> {
        if self.config.function_query.is_empty() {
            return Ok(None);
        }

        let query = Query::new(&self.config.language, self.config.function_query)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root, source);

        while let Some(m) = matches.next() {
            let mut func_node = None;
            let mut func_name = None;

            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                if capture_name == self.config.function_capture {
                    func_node = Some(capture.node);
                }
                if capture_name == self.config.func_name_capture {
                    func_name = Some(capture.node.utf8_text(source).unwrap_or(""));
                }
            }

            if let (Some(node), Some(found_name)) = (func_node, func_name) {
                if found_name == name {
                    return Ok(Some(node));
                }
            }
        }

        Ok(None)
    }

    /// Count complexity branch points within a node.
    fn count_complexity(&self, node: tree_sitter::Node, source: &[u8]) -> anyhow::Result<i32> {
        if self.config.complexity_query.is_empty() {
            return Ok(1); // Base complexity if no query configured
        }

        let query = Query::new(&self.config.language, self.config.complexity_query)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, node, source);

        let mut complexity = 1; // Base complexity
        while matches.next().is_some() {
            complexity += 1;
        }

        Ok(complexity)
    }
}

impl Parser for TreeSitterParser {
    fn parse_symbols(&self, source: &[u8]) -> anyhow::Result<Vec<Symbol>> {
        let tree = self.parse(source)?;
        let root = tree.root_node();

        if self.config.symbol_query.is_empty() {
            return Ok(vec![]);
        }

        let query = Query::new(&self.config.language, self.config.symbol_query)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root, source);

        let mut symbols = Vec::new();

        while let Some(m) = matches.next() {
            for sc in self.config.symbol_captures {
                for capture in m.captures {
                    let capture_name = query.capture_names()[capture.index as usize];
                    if capture_name == sc.name_capture {
                        let name = capture.node.utf8_text(source).unwrap_or("").to_string();
                        if !name.is_empty() {
                            symbols.push(Symbol {
                                name,
                                kind: sc.kind.to_string(),
                                file: String::new(), // Will be set by caller
                                line: capture.node.start_position().row + 1,
                            });
                        }
                    }
                }
            }
        }

        Ok(symbols)
    }

    fn complexity(&self, source: &[u8], symbol_name: &str) -> anyhow::Result<i32> {
        let tree = self.parse(source)?;
        let root = tree.root_node();

        // Find the function node
        let func_node = self.find_function(root, source, symbol_name)?;

        match func_node {
            Some(node) => self.count_complexity(node, source),
            None => Ok(0), // Symbol not found
        }
    }

    fn language(&self) -> &str {
        self.config.language_name
    }

    /// Optimized: Extract all symbols with complexity in one parse pass.
    /// This avoids re-parsing the file for each function's complexity calculation.
    fn parse_symbols_with_complexity(&self, source: &[u8]) -> anyhow::Result<Vec<SymbolWithComplexity>> {
        let tree = self.parse(source)?;
        let root = tree.root_node();

        if self.config.symbol_query.is_empty() {
            return Ok(vec![]);
        }

        // First pass: collect all symbols
        let query = Query::new(&self.config.language, self.config.symbol_query)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, root, source);

        let mut symbols = Vec::new();

        while let Some(m) = matches.next() {
            for sc in self.config.symbol_captures {
                for capture in m.captures {
                    let capture_name = query.capture_names()[capture.index as usize];
                    if capture_name == sc.name_capture {
                        let name = capture.node.utf8_text(source).unwrap_or("").to_string();
                        if !name.is_empty() {
                            symbols.push(Symbol {
                                name,
                                kind: sc.kind.to_string(),
                                file: String::new(),
                                line: capture.node.start_position().row + 1,
                            });
                        }
                    }
                }
            }
        }

        // If no function query or complexity query, return symbols without complexity
        if self.config.function_query.is_empty() || self.config.complexity_query.is_empty() {
            return Ok(symbols
                .into_iter()
                .map(|symbol| {
                    let complexity = if symbol.kind == "function" || symbol.kind == "method" {
                        Some(1) // Base complexity
                    } else {
                        None
                    };
                    SymbolWithComplexity { symbol, complexity }
                })
                .collect());
        }

        // Build a map of function names to their AST nodes for O(1) lookup
        let func_query = Query::new(&self.config.language, self.config.function_query)?;
        let mut func_cursor = QueryCursor::new();
        let mut func_matches = func_cursor.matches(&func_query, root, source);

        // Collect all function nodes in one pass
        let mut func_nodes: std::collections::HashMap<String, tree_sitter::Node> =
            std::collections::HashMap::new();

        while let Some(m) = func_matches.next() {
            let mut func_node = None;
            let mut func_name = None;

            for capture in m.captures {
                let capture_name = func_query.capture_names()[capture.index as usize];
                if capture_name == self.config.function_capture {
                    func_node = Some(capture.node);
                }
                if capture_name == self.config.func_name_capture {
                    func_name = Some(capture.node.utf8_text(source).unwrap_or("").to_string());
                }
            }

            if let (Some(node), Some(name)) = (func_node, func_name) {
                if !name.is_empty() {
                    func_nodes.insert(name, node);
                }
            }
        }

        // Compute complexity for each function/method using cached nodes
        let complexity_query = Query::new(&self.config.language, self.config.complexity_query)?;

        let result = symbols
            .into_iter()
            .map(|symbol| {
                let complexity = if symbol.kind == "function" || symbol.kind == "method" {
                    if let Some(node) = func_nodes.get(&symbol.name) {
                        // Count complexity branch points within the function node
                        let mut cc_cursor = QueryCursor::new();
                        let mut cc_matches = cc_cursor.matches(&complexity_query, *node, source);
                        let mut cc = 1; // Base complexity
                        while cc_matches.next().is_some() {
                            cc += 1;
                        }
                        Some(cc)
                    } else {
                        Some(1) // Function not found in AST, return base complexity
                    }
                } else {
                    None
                };
                SymbolWithComplexity { symbol, complexity }
            })
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    #[cfg(feature = "tree-sitter")]
    fn test_python_parser() {
        use crate::parser::languages::python;

        let parser = python::new_parser();
        let source = br#"
def hello():
    print("Hello")

class MyClass:
    def method(self):
        pass
"#;

        let symbols = parser.parse_symbols(source).unwrap();
        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == "function"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "MyClass" && s.kind == "type"));
        // Note: method extraction depends on query configuration
    }

    #[test]
    #[cfg(feature = "tree-sitter")]
    fn test_python_complexity() {
        use crate::parser::languages::python;

        let parser = python::new_parser();
        let source = br#"
def simple():
    return 1

def complex(x):
    if x > 0:
        for i in range(x):
            if i % 2 == 0:
                print(i)
    return x
"#;

        // Simple function: base complexity = 1
        let simple_cc = parser.complexity(source, "simple").unwrap();
        assert_eq!(simple_cc, 1);

        // Complex function: 1 (base) + 1 (if) + 1 (for) + 1 (if) = 4
        let complex_cc = parser.complexity(source, "complex").unwrap();
        assert!(complex_cc >= 4, "Expected >= 4, got {}", complex_cc);
    }
}

//! Language-agnostic parsing interface for symbol extraction and complexity calculation.
//!
//! This module provides:
//! - `Parser` trait: Abstract interface for language parsers
//! - `Registry`: Factory-based parser lookup by file extension
//! - Tree-sitter implementations for multiple languages

use std::collections::HashMap;
use std::sync::RwLock;

#[cfg(feature = "tree-sitter")]
pub mod treesitter;

#[cfg(feature = "tree-sitter")]
pub mod languages;

/// A symbol represents a named code element (function, method, type, const).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The symbol name (e.g., "main", "Config", "MAX_SIZE")
    pub name: String,
    /// The symbol kind: "function", "method", "type", "const", "class", "interface"
    pub kind: String,
    /// The source file path
    pub file: String,
    /// Line number (1-indexed)
    pub line: usize,
}

/// A symbol with complexity information (used for optimized god object detection).
#[derive(Debug, Clone)]
pub struct SymbolWithComplexity {
    /// The base symbol
    pub symbol: Symbol,
    /// Cyclomatic complexity (only computed for functions/methods)
    pub complexity: Option<i32>,
}

/// Parser trait for extracting symbols and calculating complexity.
pub trait Parser: Send + Sync {
    /// Extract all symbols from source code.
    fn parse_symbols(&self, source: &[u8]) -> anyhow::Result<Vec<Symbol>>;

    /// Calculate cyclomatic complexity for a named symbol.
    /// Returns 0 if the symbol is not found (not an error).
    fn complexity(&self, source: &[u8], symbol_name: &str) -> anyhow::Result<i32>;

    /// Return the language this parser handles (e.g., "go", "python").
    fn language(&self) -> &str;

    /// Extract all symbols with complexity in one pass (optimized).
    /// Default implementation calls parse_symbols + complexity for each,
    /// but tree-sitter implementation does it in one parse.
    fn parse_symbols_with_complexity(&self, source: &[u8]) -> anyhow::Result<Vec<SymbolWithComplexity>> {
        let symbols = self.parse_symbols(source)?;
        let mut result = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let complexity = if symbol.kind == "function" || symbol.kind == "method" {
                Some(self.complexity(source, &symbol.name)?)
            } else {
                None
            };
            result.push(SymbolWithComplexity { symbol, complexity });
        }
        Ok(result)
    }
}

/// Factory function type for creating parser instances.
pub type ParserFactory = fn() -> Box<dyn Parser>;

lazy_static::lazy_static! {
    /// Global parser registry mapping file extensions to parser factories.
    static ref REGISTRY: RwLock<HashMap<String, ParserFactory>> = RwLock::new(HashMap::new());
}

/// Register a parser factory for a file extension.
/// Extension should include the dot (e.g., ".go", ".py").
pub fn register(ext: &str, factory: ParserFactory) {
    let mut registry = REGISTRY.write().unwrap();
    registry.insert(ext.to_string(), factory);
}

/// Get a parser for the given file extension.
/// Returns None if no parser is registered for the extension.
pub fn for_extension(ext: &str) -> Option<Box<dyn Parser>> {
    let registry = REGISTRY.read().unwrap();
    registry.get(ext).map(|factory| factory())
}

/// Return all registered file extensions.
pub fn supported_extensions() -> Vec<String> {
    let registry = REGISTRY.read().unwrap();
    registry.keys().cloned().collect()
}

/// Initialize the parser registry with all available language parsers.
/// Call this once at startup before using parsers.
#[cfg(feature = "tree-sitter")]
pub fn init() {
    languages::register_all();
}

/// Initialize (no-op when tree-sitter is disabled).
#[cfg(not(feature = "tree-sitter"))]
pub fn init() {
    // No tree-sitter parsers available
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockParser;

    impl Parser for MockParser {
        fn parse_symbols(&self, _source: &[u8]) -> anyhow::Result<Vec<Symbol>> {
            Ok(vec![Symbol {
                name: "test".to_string(),
                kind: "function".to_string(),
                file: "test.mock".to_string(),
                line: 1,
            }])
        }

        fn complexity(&self, _source: &[u8], _symbol_name: &str) -> anyhow::Result<i32> {
            Ok(1)
        }

        fn language(&self) -> &str {
            "mock"
        }
    }

    fn mock_factory() -> Box<dyn Parser> {
        Box::new(MockParser)
    }

    #[test]
    fn test_registry() {
        register(".mock", mock_factory);

        let parser = for_extension(".mock");
        assert!(parser.is_some());

        let parser = parser.unwrap();
        assert_eq!(parser.language(), "mock");

        let symbols = parser.parse_symbols(b"test").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "test");
    }

    #[test]
    fn test_unregistered_extension() {
        let parser = for_extension(".unknown");
        assert!(parser.is_none());
    }
}

//! Core traits for language analysis.

use std::path::Path;

use super::FileFacts;

/// Holds a parsed tree-sitter tree and associated metadata.
///
/// This is kept separate from FileFacts to allow reusing the tree
/// for multiple analysis passes without re-parsing.
pub struct ParsedFile {
    /// The tree-sitter parse tree.
    pub tree: tree_sitter::Tree,
    /// The original source code (kept for node text extraction).
    pub source: Vec<u8>,
    /// The file path (for error reporting).
    pub path: String,
}

impl ParsedFile {
    /// Get the source code as a string slice.
    pub fn source_str(&self) -> &str {
        std::str::from_utf8(&self.source).unwrap_or("")
    }

    /// Get text for a tree-sitter node.
    pub fn node_text(&self, node: tree_sitter::Node) -> &str {
        node.utf8_text(&self.source).unwrap_or("")
    }
}

/// Language-specific analyzer trait.
///
/// Each language (Go, Rust, etc.) implements this trait to provide
/// AST-backed analysis capabilities.
///
/// # Thread Safety
///
/// Note: tree_sitter::Parser is not Sync, so implementations should
/// create parsers as needed or use thread-local storage.
pub trait LanguageAnalyzer: Send + Sync {
    /// Returns the language identifier (e.g., "go", "rust").
    fn language_id(&self) -> &'static str;

    /// Returns glob patterns for files this analyzer handles.
    ///
    /// Examples: `["**/*.go"]`, `["**/*.rs"]`
    fn file_globs(&self) -> &'static [&'static str];

    /// Returns file extensions this analyzer handles (without dot).
    ///
    /// Examples: `["go"]`, `["rs"]`
    fn file_extensions(&self) -> &'static [&'static str];

    /// Parse a source file into a tree-sitter tree.
    ///
    /// Returns an error if parsing fails completely (e.g., wrong language).
    /// Partial parse errors are still returned as a valid tree with ERROR nodes.
    fn parse(&self, path: &Path, source: &[u8]) -> anyhow::Result<ParsedFile>;

    /// Extract all facts from a parsed file.
    ///
    /// This is the main analysis entry point. It extracts:
    /// - Declarations (functions, methods, types, constants)
    /// - Imports
    /// - Control flow information
    /// - Function body details for stub detection
    fn extract_facts(&self, parsed: &ParsedFile) -> anyhow::Result<FileFacts>;

    /// Check if this analyzer handles the given file extension.
    fn handles_extension(&self, ext: &str) -> bool {
        self.file_extensions().contains(&ext)
    }
}

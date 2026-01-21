//! Hollowcheck - AI output quality gate system.
//!
//! Hollowcheck validates AI-generated code against quality contracts.
//! It detects "hollow" code - implementations that look complete but lack
//! real functionality: stub implementations, placeholder data, unfinished
//! work markers, and functions with suspiciously low complexity.
//!
//! # Architecture
//!
//! The codebase uses tree-sitter for AST-based analysis:
//!
//! - `analysis`: Core AST analysis module with language analyzers
//! - `parser`: Legacy tree-sitter parsers (being migrated to `analysis`)
//! - `detect`: Detection rules that consume AST-derived facts
//! - `contract`: YAML contract schema definitions
//! - `report`: Output formatting (text, JSON)
//! - `score`: Hollowness score calculation
//!
//! # Adding a New Language
//!
//! See `src/analysis/languages/` for examples. Implement `LanguageAnalyzer`
//! trait and register in `languages/mod.rs`.

pub mod analysis;
pub mod cli;
pub mod contract;
pub mod detect;
pub mod parser;
pub mod registry;
pub mod report;
pub mod score;

pub use analysis::{
    register_analyzers, AnalysisContext, Declaration, DeclarationKind, FileFacts,
    GoAnalyzer, LanguageAnalyzer, RustAnalyzer, StubDetector, StubFinding,
};
pub use contract::Contract;
pub use detect::{DetectionResult, Runner, Violation};
pub use parser::{for_extension, init as init_parsers, Parser, Symbol};
pub use registry::{RegistryClient, RegistryType};
pub use score::HollownessScore;

/// Initialize all subsystems.
///
/// Call this once at startup.
pub fn init() {
    // Initialize legacy parser registry
    init_parsers();
    // Initialize new analyzer registry
    register_analyzers();
}

//! AST-backed code analysis module.
//!
//! This module provides a language-agnostic interface for extracting "facts"
//! from source code using tree-sitter. Facts include:
//! - Declarations (functions, methods, types, constants)
//! - Imports/dependencies
//! - Control flow information for complexity calculation
//! - Function body analysis for stub detection
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────┐     ┌───────────────┐
//! │ Source Files    │────▶│ Analyzers    │────▶│ FileFacts     │
//! └─────────────────┘     │ (Go, Rust)   │     │ (Declarations,│
//!                         └──────────────┘     │  Imports, etc)│
//!                                              └───────────────┘
//!                                                      │
//!                                                      ▼
//!                         ┌──────────────┐     ┌───────────────┐
//!                         │ Detection    │◀────│AnalysisContext│
//!                         │ Rules        │     │ (Cached Facts)│
//!                         └──────────────┘     └───────────────┘
//! ```
//!
//! # Adding a New Language
//!
//! 1. Create a new module in `src/analysis/languages/` (e.g., `python.rs`)
//! 2. Implement `LanguageAnalyzer` trait
//! 3. Define tree-sitter queries for symbol extraction
//! 4. Register the analyzer in `languages/mod.rs`
//!
//! See `languages/go.rs` for a reference implementation.

mod context;
mod facts;
mod languages;
mod stubs;
mod traits;

pub use context::AnalysisContext;
pub use facts::{
    ControlFlowInfo, Declaration, DeclarationKind, FileFacts, FunctionBody, Import, Span,
};
pub use languages::{
    get_analyzer, register_analyzers, CAnalyzer, CppAnalyzer, GoAnalyzer, JavaAnalyzer,
    JavaScriptAnalyzer, PythonAnalyzer, RustAnalyzer, ScalaAnalyzer, SwiftAnalyzer,
    TypeScriptAnalyzer,
};
pub use stubs::{HollowBodyKind, StubDetector, StubDetectorConfig, StubFinding};
pub use traits::{LanguageAnalyzer, ParsedFile};

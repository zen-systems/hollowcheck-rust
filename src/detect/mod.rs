//! Detection module for quality issues in code.
//!
//! This module provides detection rules for various code quality issues:
//!
//! - **AST-backed rules** (tree-sitter):
//!   - `stubs`: Hollow function detection (empty, panic-only, TODO-only)
//!   - `symbols`: Required symbol verification
//!   - `complexity`: Cyclomatic complexity checking
//!
//! - **Text-based rules**:
//!   - `patterns`: Forbidden pattern matching
//!   - `todos`: Hollow TODO comment detection
//!   - `mocks`: Mock data detection

mod complexity;
mod dependencies;
mod files;
mod god_objects;
mod imports;
pub mod manifest;
mod mocks;
mod patterns;
mod runner;
mod stdlib;
mod stubs;
mod suppress;
mod symbols;
mod todos;
mod types;

pub use complexity::detect_low_complexity;
pub use dependencies::{detect_hallucinated_dependencies, DependencyValidator};
pub use manifest::{
    detect_manifest_type, GoManifest, HomeAssistantManifest, ManifestProvider, ManifestStats,
    ManifestType, NoManifest, PythonManifest,
};
pub use files::detect_missing_files;
pub use god_objects::{detect_god_objects, GodObjectConfig};
pub use imports::{extract_imports, ImportedDependency};
pub use mocks::detect_mock_data;
pub use patterns::detect_forbidden_patterns;
pub use runner::Runner;
pub use stubs::{detect_stub_functions, StubDetectionConfig};
pub use suppress::{
    collect_suppressions, filter_suppressed, parse_suppressions, SuppressedViolation, Suppression,
    SuppressionType,
};
pub use symbols::{detect_missing_symbols, detect_missing_tests};
pub use todos::detect_hollow_todos;
pub use types::{DetectionResult, Severity, Violation, ViolationRule};

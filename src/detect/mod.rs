//! Detection module for quality issues in code.

mod complexity;
mod dependencies;
mod files;
mod god_objects;
mod imports;
mod mocks;
mod patterns;
mod runner;
mod stdlib;
mod suppress;
mod symbols;
mod todos;
mod types;

pub use complexity::detect_low_complexity;
pub use dependencies::detect_hallucinated_dependencies;
pub use files::detect_missing_files;
pub use god_objects::{detect_god_objects, GodObjectConfig};
pub use imports::{extract_imports, ImportedDependency};
pub use mocks::detect_mock_data;
pub use patterns::detect_forbidden_patterns;
pub use runner::Runner;
pub use suppress::{
    collect_suppressions, filter_suppressed, parse_suppressions, SuppressedViolation, Suppression,
    SuppressionType,
};
pub use symbols::{detect_missing_symbols, detect_missing_tests};
pub use todos::detect_hollow_todos;
pub use types::{DetectionResult, Severity, Violation, ViolationRule};

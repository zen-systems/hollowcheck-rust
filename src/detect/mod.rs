//! Detection module for quality issues in code.

mod complexity;
mod dependencies;
mod files;
mod imports;
mod mocks;
mod patterns;
mod runner;
mod suppress;
mod symbols;
mod types;

pub use complexity::detect_low_complexity;
pub use dependencies::detect_hallucinated_dependencies;
pub use files::detect_missing_files;
pub use imports::{extract_imports, ImportedDependency};
pub use mocks::detect_mock_data;
pub use patterns::detect_forbidden_patterns;
pub use runner::Runner;
pub use suppress::{
    collect_suppressions, filter_suppressed, parse_suppressions, Suppression, SuppressionType,
    SuppressedViolation,
};
pub use symbols::{detect_missing_symbols, detect_missing_tests};
pub use types::{DetectionResult, Severity, Violation, ViolationRule};

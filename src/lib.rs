//! Hollowcheck - AI output quality gate system.
//!
//! Hollowcheck validates AI-generated code against quality contracts.
//! It detects "hollow" code - implementations that look complete but lack
//! real functionality: stub implementations, placeholder data, unfinished
//! work markers, and functions with suspiciously low complexity.

pub mod cli;
pub mod contract;
pub mod detect;
pub mod parser;
pub mod registry;
pub mod report;
pub mod score;

pub use contract::Contract;
pub use detect::{DetectionResult, Runner, Violation};
pub use parser::{for_extension, init as init_parsers, Parser, Symbol};
pub use registry::{RegistryClient, RegistryType};
pub use score::HollownessScore;

//! Language-specific tree-sitter configurations.
//!
//! Each language module provides:
//! - Symbol extraction queries
//! - Complexity counting queries
//! - Function finding queries
//! - Factory function for creating parsers

#[cfg(feature = "tree-sitter")]
pub mod go;
#[cfg(feature = "tree-sitter")]
pub mod java;
#[cfg(feature = "tree-sitter")]
pub mod python;
#[cfg(feature = "tree-sitter")]
pub mod typescript;

/// Register all available language parsers.
#[cfg(feature = "tree-sitter")]
pub fn register_all() {
    python::register();
    typescript::register();
    java::register();
    go::register();
}

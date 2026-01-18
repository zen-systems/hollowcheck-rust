//! Language-specific tree-sitter configurations.
//!
//! Each language module provides:
//! - Symbol extraction queries
//! - Complexity counting queries
//! - Function finding queries
//! - Factory function for creating parsers

#[cfg(feature = "tree-sitter")]
pub mod c;
#[cfg(feature = "tree-sitter")]
pub mod cpp;
#[cfg(feature = "tree-sitter")]
pub mod go;
#[cfg(feature = "tree-sitter")]
pub mod java;
#[cfg(feature = "tree-sitter")]
pub mod javascript;
#[cfg(feature = "tree-sitter")]
pub mod kotlin;
#[cfg(feature = "tree-sitter")]
pub mod python;
#[cfg(feature = "tree-sitter")]
pub mod rust_lang;
#[cfg(feature = "tree-sitter")]
pub mod typescript;

/// Register all available language parsers.
#[cfg(feature = "tree-sitter")]
pub fn register_all() {
    c::register();
    cpp::register();
    go::register();
    java::register();
    javascript::register();
    kotlin::register();
    python::register();
    rust_lang::register();
    typescript::register();
}

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
// HCL disabled: tree-sitter-hcl requires ABI 15, tree-sitter 0.24 supports 13-14
// #[cfg(feature = "tree-sitter")]
// pub mod hcl;
#[cfg(feature = "tree-sitter")]
pub mod java;
#[cfg(feature = "tree-sitter")]
pub mod javascript;
// Kotlin disabled: tree-sitter-kotlin requires tree-sitter 0.20, conflicts with 0.24
// #[cfg(feature = "tree-sitter")]
// pub mod kotlin;
#[cfg(feature = "tree-sitter")]
pub mod python;
#[cfg(feature = "tree-sitter")]
pub mod rust_lang;
#[cfg(feature = "tree-sitter")]
pub mod scala;
#[cfg(feature = "tree-sitter")]
pub mod swift;
#[cfg(feature = "tree-sitter")]
pub mod typescript;

/// Register all available language parsers.
#[cfg(feature = "tree-sitter")]
pub fn register_all() {
    c::register();
    cpp::register();
    go::register();
    // hcl::register(); // Disabled: requires ABI 15
    java::register();
    javascript::register();
    // kotlin::register(); // Disabled: incompatible tree-sitter version
    python::register();
    rust_lang::register();
    scala::register();
    swift::register();
    typescript::register();
}

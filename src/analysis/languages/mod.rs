//! Language-specific analyzer implementations.

mod c;
mod cpp;
mod go;
mod java;
mod javascript;
mod python;
mod rust_lang;
mod scala;
mod swift;
mod typescript;

pub use c::CAnalyzer;
pub use cpp::CppAnalyzer;
pub use go::GoAnalyzer;
pub use java::JavaAnalyzer;
pub use javascript::JavaScriptAnalyzer;
pub use python::PythonAnalyzer;
pub use rust_lang::RustAnalyzer;
pub use scala::ScalaAnalyzer;
pub use swift::SwiftAnalyzer;
pub use typescript::TypeScriptAnalyzer;

use super::LanguageAnalyzer;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Static storage for C analyzer.
static C_ANALYZER: OnceCell<CAnalyzer> = OnceCell::new();

/// Static storage for C++ analyzer.
static CPP_ANALYZER: OnceCell<CppAnalyzer> = OnceCell::new();

/// Static storage for Go analyzer.
static GO_ANALYZER: OnceCell<GoAnalyzer> = OnceCell::new();

/// Static storage for Java analyzer.
static JAVA_ANALYZER: OnceCell<JavaAnalyzer> = OnceCell::new();

/// Static storage for JavaScript analyzer.
static JAVASCRIPT_ANALYZER: OnceCell<JavaScriptAnalyzer> = OnceCell::new();

/// Static storage for Python analyzer.
static PYTHON_ANALYZER: OnceCell<PythonAnalyzer> = OnceCell::new();

/// Static storage for Rust analyzer.
static RUST_ANALYZER: OnceCell<RustAnalyzer> = OnceCell::new();

/// Static storage for Scala analyzer.
static SCALA_ANALYZER: OnceCell<ScalaAnalyzer> = OnceCell::new();

/// Static storage for Swift analyzer.
static SWIFT_ANALYZER: OnceCell<SwiftAnalyzer> = OnceCell::new();

/// Static storage for TypeScript analyzer.
static TYPESCRIPT_ANALYZER: OnceCell<TypeScriptAnalyzer> = OnceCell::new();

/// Whether analyzers have been registered.
static REGISTERED: AtomicBool = AtomicBool::new(false);

/// Register all available language analyzers.
///
/// Call this once at startup before using analyzers.
/// This is idempotent - calling it multiple times is safe.
pub fn register_analyzers() {
    if REGISTERED.swap(true, Ordering::SeqCst) {
        return; // Already registered
    }

    C_ANALYZER.get_or_init(CAnalyzer::new);
    CPP_ANALYZER.get_or_init(CppAnalyzer::new);
    GO_ANALYZER.get_or_init(GoAnalyzer::new);
    JAVA_ANALYZER.get_or_init(JavaAnalyzer::new);
    JAVASCRIPT_ANALYZER.get_or_init(JavaScriptAnalyzer::new);
    PYTHON_ANALYZER.get_or_init(PythonAnalyzer::new);
    RUST_ANALYZER.get_or_init(RustAnalyzer::new);
    SCALA_ANALYZER.get_or_init(ScalaAnalyzer::new);
    SWIFT_ANALYZER.get_or_init(SwiftAnalyzer::new);
    TYPESCRIPT_ANALYZER.get_or_init(TypeScriptAnalyzer::new);
}

/// Get an analyzer for the given file extension.
///
/// Returns None if no analyzer is registered for the extension.
pub fn get_analyzer(ext: &str) -> Option<&'static dyn LanguageAnalyzer> {
    // Ensure analyzers are registered
    register_analyzers();

    match ext {
        // C
        "c" | "h" => C_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // C++
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => {
            CPP_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer)
        }
        // Go
        "go" => GO_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // Java
        "java" => JAVA_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // JavaScript
        "js" | "jsx" | "mjs" => {
            JAVASCRIPT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer)
        }
        // Python
        "py" => PYTHON_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // Rust
        "rs" => RUST_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // Scala
        "scala" | "sc" => SCALA_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // Swift
        "swift" => SWIFT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        // TypeScript
        "ts" | "tsx" | "mts" => {
            TYPESCRIPT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer)
        }
        _ => None,
    }
}

/// Get an analyzer by language ID.
#[allow(dead_code)]
pub fn get_analyzer_by_id(lang_id: &str) -> Option<&'static dyn LanguageAnalyzer> {
    // Ensure analyzers are registered
    register_analyzers();

    match lang_id {
        "c" => C_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "cpp" => CPP_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "go" => GO_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "java" => JAVA_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "javascript" => JAVASCRIPT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "python" => PYTHON_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "rust" => RUST_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "scala" => SCALA_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "swift" => SWIFT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        "typescript" => TYPESCRIPT_ANALYZER.get().map(|a| a as &'static dyn LanguageAnalyzer),
        _ => None,
    }
}

/// Get all registered language IDs.
#[allow(dead_code)]
pub fn registered_languages() -> Vec<String> {
    vec![
        "c".to_string(),
        "cpp".to_string(),
        "go".to_string(),
        "java".to_string(),
        "javascript".to_string(),
        "python".to_string(),
        "rust".to_string(),
        "scala".to_string(),
        "swift".to_string(),
        "typescript".to_string(),
    ]
}

/// Get all registered file extensions.
#[allow(dead_code)]
pub fn registered_extensions() -> Vec<String> {
    vec![
        "c".to_string(),
        "h".to_string(),
        "cpp".to_string(),
        "cc".to_string(),
        "cxx".to_string(),
        "hpp".to_string(),
        "hh".to_string(),
        "go".to_string(),
        "java".to_string(),
        "js".to_string(),
        "jsx".to_string(),
        "mjs".to_string(),
        "py".to_string(),
        "rs".to_string(),
        "scala".to_string(),
        "sc".to_string(),
        "swift".to_string(),
        "ts".to_string(),
        "tsx".to_string(),
        "mts".to_string(),
    ]
}

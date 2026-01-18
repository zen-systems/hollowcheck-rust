//! Standard library detection for multiple programming languages.
//!
//! This module provides efficient detection of standard library modules by:
//! 1. Checking an in-memory cache
//! 2. Checking a disk cache
//! 3. Querying the actual language runtime
//! 4. Falling back to embedded minimal lists
//!
//! The runtime query approach ensures accuracy for the user's actual
//! installed version, while caching makes subsequent lookups O(1).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

mod fallback;

/// Cache entry with metadata.
struct StdlibCache {
    modules: HashSet<String>,
    #[allow(dead_code)]
    version: String,
    timestamp: SystemTime,
}

/// Global in-memory cache.
static STDLIB_CACHE: std::sync::LazyLock<RwLock<HashMap<StdlibLanguage, StdlibCache>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Cache TTL - 24 hours.
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Supported languages for stdlib detection.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum StdlibLanguage {
    Python,
    JavaScript, // Node.js
    Go,
    Rust,
}

/// Check if a module is part of the standard library.
pub fn is_stdlib(lang: StdlibLanguage, module: &str) -> bool {
    // 1. Check in-memory cache
    if let Some(result) = check_memory_cache(lang, module) {
        return result;
    }

    // 2. Load from disk cache or query runtime
    let stdlib = get_or_refresh_stdlib(lang);
    stdlib.contains(module)
}

/// Check in-memory cache.
fn check_memory_cache(lang: StdlibLanguage, module: &str) -> Option<bool> {
    let cache = STDLIB_CACHE.read().ok()?;
    let entry = cache.get(&lang)?;

    // Check if still valid
    if entry.timestamp.elapsed().ok()? < CACHE_TTL {
        return Some(entry.modules.contains(module));
    }
    None
}

/// Get stdlib set, refreshing if needed.
fn get_or_refresh_stdlib(lang: StdlibLanguage) -> HashSet<String> {
    // In CI, prefer embedded fallback to avoid flaky runtime detection
    if is_ci() {
        if let Some(cached) = load_disk_cache(lang) {
            update_memory_cache(lang, cached.clone(), "cached".to_string());
            return cached;
        }
        let fallback = fallback::get_embedded_fallback(lang);
        update_memory_cache(lang, fallback.clone(), "embedded".to_string());
        return fallback;
    }

    // Try disk cache first
    if let Some(cached) = load_disk_cache(lang) {
        update_memory_cache(lang, cached.clone(), "cached".to_string());
        return cached;
    }

    // Query runtime
    let (modules, version) = query_runtime(lang).unwrap_or_else(|| {
        (
            fallback::get_embedded_fallback(lang),
            "embedded".to_string(),
        )
    });

    // Save to disk cache
    save_disk_cache(lang, &modules, &version);

    // Update memory cache
    update_memory_cache(lang, modules.clone(), version);

    modules
}

/// Check if we're in a CI environment.
fn is_ci() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("CIRCLECI").is_ok()
}

/// Get cache directory.
fn cache_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "hollowcheck")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join(".hollowcheck_cache"))
}

/// Disk cache filename.
fn cache_file(lang: StdlibLanguage) -> PathBuf {
    let name = match lang {
        StdlibLanguage::Python => "python_stdlib.txt",
        StdlibLanguage::JavaScript => "node_builtins.txt",
        StdlibLanguage::Go => "go_stdlib.txt",
        StdlibLanguage::Rust => "rust_stdlib.txt",
    };
    cache_dir().join(name)
}

/// Load from disk cache if fresh.
fn load_disk_cache(lang: StdlibLanguage) -> Option<HashSet<String>> {
    let path = cache_file(lang);
    let metadata = fs::metadata(&path).ok()?;

    // Check if stale
    if metadata.modified().ok()?.elapsed().ok()? > CACHE_TTL {
        return None;
    }

    let content = fs::read_to_string(&path).ok()?;
    let modules: HashSet<String> = content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();

    if modules.is_empty() {
        return None;
    }

    Some(modules)
}

/// Save to disk cache.
fn save_disk_cache(lang: StdlibLanguage, modules: &HashSet<String>, version: &str) {
    let path = cache_file(lang);

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut content = format!("# version: {}\n", version);
    let mut sorted: Vec<_> = modules.iter().collect();
    sorted.sort();
    for m in sorted {
        content.push_str(m);
        content.push('\n');
    }

    let _ = fs::write(path, content);
}

/// Update in-memory cache.
fn update_memory_cache(lang: StdlibLanguage, modules: HashSet<String>, version: String) {
    if let Ok(mut cache) = STDLIB_CACHE.write() {
        cache.insert(
            lang,
            StdlibCache {
                modules,
                version,
                timestamp: SystemTime::now(),
            },
        );
    }
}

/// Query the actual runtime for stdlib modules.
fn query_runtime(lang: StdlibLanguage) -> Option<(HashSet<String>, String)> {
    match lang {
        StdlibLanguage::Python => query_python_stdlib(),
        StdlibLanguage::JavaScript => query_node_builtins(),
        StdlibLanguage::Go => query_go_stdlib(),
        StdlibLanguage::Rust => Some((fallback::rust_stdlib(), "builtin".to_string())),
    }
}

/// Query Python stdlib using sys.stdlib_module_names (Python 3.10+).
fn query_python_stdlib() -> Option<(HashSet<String>, String)> {
    let python = find_python()?;

    // Get version
    let version_output = Command::new(&python).args(["--version"]).output().ok()?;
    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string();

    // Get stdlib modules
    // sys.stdlib_module_names is Python 3.10+
    // Fall back to sys.builtin_module_names + known stdlib for older versions
    let script = r#"
import sys
if hasattr(sys, 'stdlib_module_names'):
    print('\n'.join(sorted(sys.stdlib_module_names)))
else:
    # Fallback for Python < 3.10
    import pkgutil
    import os
    stdlib_path = os.path.dirname(os.__file__)
    modules = set(sys.builtin_module_names)
    for importer, modname, ispkg in pkgutil.iter_modules([stdlib_path]):
        modules.add(modname)
    print('\n'.join(sorted(modules)))
"#;

    let output = Command::new(&python).args(["-c", script]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let modules: HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    if modules.is_empty() {
        return None;
    }

    Some((modules, version))
}

/// Find Python executable.
fn find_python() -> Option<String> {
    for cmd in ["python3", "python"] {
        if let Ok(output) = Command::new(cmd).arg("--version").output() {
            if output.status.success() {
                return Some(cmd.to_string());
            }
        }
    }
    None
}

/// Query Node.js builtin modules.
fn query_node_builtins() -> Option<(HashSet<String>, String)> {
    // Get version
    let version_output = Command::new("node").args(["--version"]).output().ok()?;

    if !version_output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string();

    // Get builtin modules
    let output = Command::new("node")
        .args([
            "-e",
            "console.log(require('module').builtinModules.join('\\n'))",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let mut modules: HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    if modules.is_empty() {
        return None;
    }

    // Also add node: prefixed versions
    let prefixed: Vec<String> = modules.iter().map(|m| format!("node:{}", m)).collect();
    modules.extend(prefixed);

    Some((modules, version))
}

/// Query Go stdlib packages.
fn query_go_stdlib() -> Option<(HashSet<String>, String)> {
    // Get version
    let version_output = Command::new("go").args(["version"]).output().ok()?;

    if !version_output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string();

    // Get stdlib packages
    let output = Command::new("go").args(["list", "std"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let modules: HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    if modules.is_empty() {
        return None;
    }

    Some((modules, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_stdlib_detection() {
        // These should always be stdlib (in embedded fallback at minimum)
        assert!(is_stdlib(StdlibLanguage::Python, "os"));
        assert!(is_stdlib(StdlibLanguage::Python, "sys"));
        assert!(is_stdlib(StdlibLanguage::Python, "json"));

        // These should never be stdlib
        assert!(!is_stdlib(StdlibLanguage::Python, "requests"));
        assert!(!is_stdlib(StdlibLanguage::Python, "flask"));
        assert!(!is_stdlib(
            StdlibLanguage::Python,
            "nonexistent-package-xyz"
        ));
    }

    #[test]
    fn test_node_builtin_detection() {
        assert!(is_stdlib(StdlibLanguage::JavaScript, "fs"));
        assert!(is_stdlib(StdlibLanguage::JavaScript, "path"));
        assert!(is_stdlib(StdlibLanguage::JavaScript, "node:fs"));

        assert!(!is_stdlib(StdlibLanguage::JavaScript, "express"));
        assert!(!is_stdlib(StdlibLanguage::JavaScript, "lodash"));
    }

    #[test]
    fn test_go_stdlib_detection() {
        assert!(is_stdlib(StdlibLanguage::Go, "fmt"));
        assert!(is_stdlib(StdlibLanguage::Go, "net/http"));

        assert!(!is_stdlib(StdlibLanguage::Go, "github.com/gorilla/mux"));
    }

    #[test]
    fn test_rust_stdlib_detection() {
        assert!(is_stdlib(StdlibLanguage::Rust, "std"));
        assert!(is_stdlib(StdlibLanguage::Rust, "core"));

        assert!(!is_stdlib(StdlibLanguage::Rust, "serde"));
        assert!(!is_stdlib(StdlibLanguage::Rust, "tokio"));
    }

    #[test]
    fn test_cache_performance() {
        // First call loads the stdlib
        let lang = StdlibLanguage::Python;
        let _ = is_stdlib(lang, "os");

        // Subsequent calls should be fast (cached)
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = is_stdlib(lang, "os");
        }
        let elapsed = start.elapsed();

        // 1000 cached lookups should be < 50ms (being generous for CI)
        assert!(elapsed.as_millis() < 50, "Cache too slow: {:?}", elapsed);
    }
}

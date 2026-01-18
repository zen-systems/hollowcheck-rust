//! Detection of missing required symbols and tests.
//!
//! This module provides symbol detection with two strategies:
//! 1. Tree-sitter based AST parsing (when the feature is enabled)
//! 2. Regex-based fallback (always available)

use crate::contract::{RequiredSymbol, RequiredTest, SymbolKind};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Information about a found symbol.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
}

/// Check that all required symbols exist in the codebase.
pub fn detect_missing_symbols<P1: AsRef<Path>, P2: AsRef<Path>>(
    base_dir: P1,
    files: &[P2],
    symbols: &[RequiredSymbol],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    if symbols.is_empty() {
        return Ok(result);
    }

    let base = base_dir.as_ref();

    // Build a map of found symbols by file
    let mut found_symbols: HashMap<String, Vec<SymbolInfo>> = HashMap::new();

    for file in files {
        let path = file.as_ref();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        let syms = extract_symbols(path, &ext_with_dot)?;

        // Normalize file path relative to base_dir for matching
        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        found_symbols.insert(rel_path, syms);
        result.scanned += 1;
    }

    // Check each required symbol
    for req in symbols {
        let found = found_symbols
            .get(&req.file)
            .map(|syms| {
                syms.iter()
                    .any(|s| s.name == req.name && s.kind == req.kind)
            })
            .unwrap_or(false);

        if !found {
            result.add_violation(Violation {
                rule: ViolationRule::MissingSymbol,
                message: format!("required {} {:?} not found", req.kind, req.name),
                file: req.file.clone(),
                line: 0,
                severity: Severity::Error,
            });
        }
    }

    Ok(result)
}

/// Check that all required test functions exist.
pub fn detect_missing_tests<P1: AsRef<Path>, P2: AsRef<Path>>(
    base_dir: P1,
    files: &[P2],
    tests: &[RequiredTest],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    if tests.is_empty() {
        return Ok(result);
    }

    let base = base_dir.as_ref();

    // Build a map of found test functions by file
    let mut found_tests: HashMap<String, Vec<String>> = HashMap::new();

    // Only parse test files
    for file in files {
        let path = file.as_ref();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Only parse Go test files for now
        if !file_name.ends_with("_test.go") {
            continue;
        }

        let syms = extract_symbols(path, ".go")?;
        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let test_names: Vec<String> = syms
            .into_iter()
            .filter(|s| s.kind == SymbolKind::Function && s.name.starts_with("Test"))
            .map(|s| s.name)
            .collect();

        found_tests.insert(rel_path, test_names);
    }

    // Check each required test
    for req in tests {
        let found = if let Some(ref file) = req.file {
            // Look in specific file
            found_tests
                .get(file)
                .map(|tests| tests.iter().any(|t| t == &req.name))
                .unwrap_or(false)
        } else {
            // Look in any test file
            found_tests
                .values()
                .any(|tests| tests.iter().any(|t| t == &req.name))
        };

        if !found {
            let file = req
                .file
                .clone()
                .unwrap_or_else(|| "(any test file)".to_string());
            result.add_violation(Violation {
                rule: ViolationRule::MissingTest,
                message: format!("required test {:?} not found", req.name),
                file,
                line: 0,
                severity: Severity::Error,
            });
        }
    }

    Ok(result)
}

/// Extract symbols from a file, using tree-sitter if available.
fn extract_symbols(path: &Path, ext: &str) -> anyhow::Result<Vec<SymbolInfo>> {
    // Try tree-sitter parser first
    #[cfg(feature = "tree-sitter")]
    if let Some(parser) = crate::parser::for_extension(ext) {
        let source = fs::read(path)?;
        let symbols = parser.parse_symbols(&source)?;
        let file_str = path.to_string_lossy().to_string();
        return Ok(symbols
            .into_iter()
            .map(|s| SymbolInfo {
                name: s.name,
                kind: kind_from_str(&s.kind),
                file: file_str.clone(),
                line: s.line,
            })
            .collect());
    }

    // Fall back to regex-based extraction
    match ext {
        ".go" => extract_symbols_go_regex(path),
        ".rs" => extract_symbols_rust_regex(path),
        ".py" => extract_symbols_python_regex(path),
        ".js" | ".ts" | ".jsx" | ".tsx" => extract_symbols_js_regex(path),
        ".java" => extract_symbols_java_regex(path),
        _ => Ok(vec![]),
    }
}

/// Convert a string kind to SymbolKind enum.
fn kind_from_str(s: &str) -> SymbolKind {
    match s {
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "type" | "class" | "interface" | "enum" => SymbolKind::Type,
        "const" => SymbolKind::Const,
        _ => SymbolKind::Function, // Default
    }
}

/// Extract symbols from a Go file using regex patterns.
fn extract_symbols_go_regex(file_path: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    // Function pattern: func Name( or func (receiver) Name(
    let func_re = Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?(\w+)\s*\(")?;
    // Type pattern: type Name struct/interface/etc
    let type_re = Regex::new(r"(?m)^type\s+(\w+)\s+")?;
    // Const pattern: const Name = or const ( Name = )
    let const_re = Regex::new(r"(?m)(?:^const\s+(\w+)\s*=|^\s+(\w+)\s*=)")?;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;

        // Check for functions
        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                // Determine if it's a method (has receiver)
                let is_method = line.contains("func (");
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: if is_method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        // Check for types
        if let Some(caps) = type_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        // Check for constants
        if let Some(caps) = const_re.captures(line) {
            let name = caps.get(1).or_else(|| caps.get(2));
            if let Some(n) = name {
                symbols.push(SymbolInfo {
                    name: n.as_str().to_string(),
                    kind: SymbolKind::Const,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }
    }

    Ok(symbols)
}

/// Extract symbols from a Rust file using regex patterns.
fn extract_symbols_rust_regex(file_path: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    // Function pattern: fn name( or pub fn name(
    let func_re = Regex::new(r"(?m)^\s*(?:pub\s+)?fn\s+(\w+)\s*[<(]")?;
    // Struct/enum pattern
    let type_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(?:struct|enum|trait|type)\s+(\w+)")?;
    // Const pattern
    let const_re = Regex::new(r"(?m)^\s*(?:pub\s+)?const\s+(\w+)\s*:")?;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Function,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = type_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = const_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Const,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }
    }

    Ok(symbols)
}

/// Extract symbols from a Python file using regex patterns.
fn extract_symbols_python_regex(file_path: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    // Function pattern: def name(
    let func_re = Regex::new(r"(?m)^(?:\s*)def\s+(\w+)\s*\(")?;
    // Class pattern: class Name
    let class_re = Regex::new(r"(?m)^class\s+(\w+)")?;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                // Check if it's a method (indented) or function (not indented)
                let is_method = line.starts_with(' ') || line.starts_with('\t');
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: if is_method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = class_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }
    }

    Ok(symbols)
}

/// Extract symbols from a JavaScript/TypeScript file using regex patterns.
fn extract_symbols_js_regex(file_path: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    // Function patterns
    let func_re = Regex::new(r"(?m)(?:^|\s)(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*[<(]")?;
    let arrow_re = Regex::new(
        r"(?m)(?:^|\s)(?:export\s+)?(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>",
    )?;
    // Class pattern
    let class_re = Regex::new(r"(?m)(?:^|\s)(?:export\s+)?class\s+(\w+)")?;
    // Type/interface pattern (TypeScript)
    let type_re = Regex::new(r"(?m)(?:^|\s)(?:export\s+)?(?:type|interface)\s+(\w+)")?;
    // Const pattern
    let const_re = Regex::new(r"(?m)(?:^|\s)(?:export\s+)?const\s+(\w+)\s*=")?;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Function,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = arrow_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Function,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = class_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = type_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        // Only add const if not already captured as a function (arrow)
        if !arrow_re.is_match(line) {
            if let Some(caps) = const_re.captures(line) {
                if let Some(name) = caps.get(1) {
                    symbols.push(SymbolInfo {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Const,
                        file: file_str.clone(),
                        line: line_number,
                    });
                }
            }
        }
    }

    Ok(symbols)
}

/// Extract symbols from a Java file using regex patterns.
fn extract_symbols_java_regex(file_path: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    // Class pattern
    let class_re = Regex::new(r"(?m)(?:public\s+)?(?:abstract\s+)?class\s+(\w+)")?;
    // Interface pattern
    let interface_re = Regex::new(r"(?m)(?:public\s+)?interface\s+(\w+)")?;
    // Enum pattern
    let enum_re = Regex::new(r"(?m)(?:public\s+)?enum\s+(\w+)")?;
    // Method pattern
    let method_re = Regex::new(
        r"(?m)\s+(?:public|private|protected)?\s*(?:static\s+)?(?:\w+(?:<[^>]+>)?)\s+(\w+)\s*\(",
    )?;

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = class_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = interface_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = enum_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Type,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }

        if let Some(caps) = method_re.captures(line) {
            if let Some(name) = caps.get(1) {
                symbols.push(SymbolInfo {
                    name: name.as_str().to_string(),
                    kind: SymbolKind::Method,
                    file: file_str.clone(),
                    line: line_number,
                });
            }
        }
    }

    Ok(symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_symbols_go_regex() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        std::fs::write(
            &file_path,
            r#"
package main

const Version = "1.0"

type Handler struct{}

func (h *Handler) Handle() {}

func main() {}
"#,
        )
        .unwrap();

        let symbols = extract_symbols_go_regex(&file_path).unwrap();
        assert!(symbols
            .iter()
            .any(|s| s.name == "Version" && s.kind == SymbolKind::Const));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Handler" && s.kind == SymbolKind::Type));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Handle" && s.kind == SymbolKind::Method));
        assert!(symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_detect_missing_symbols() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        std::fs::write(
            &file_path,
            r#"
package main

func main() {}
"#,
        )
        .unwrap();

        let symbols = vec![
            RequiredSymbol {
                name: "main".to_string(),
                kind: SymbolKind::Function,
                file: "main.go".to_string(),
            },
            RequiredSymbol {
                name: "Handler".to_string(),
                kind: SymbolKind::Type,
                file: "main.go".to_string(),
            },
        ];

        let result = detect_missing_symbols(temp.path(), &[&file_path], &symbols).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("Handler"));
    }
}

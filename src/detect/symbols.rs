//! Detection of missing required symbols and tests.
//!
//! This module uses AST-backed analysis via the AnalysisContext to extract
//! symbols from source files. For supported languages, it uses tree-sitter
//! for accurate parsing. Unsupported extensions result in explicit failures.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::analysis::{get_analyzer, AnalysisContext, DeclarationKind, FileFacts};
use crate::contract::{RequiredSymbol, RequiredTest, SymbolKind};

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

/// Convert from analysis DeclarationKind to contract SymbolKind.
fn declaration_kind_to_symbol_kind(kind: DeclarationKind) -> SymbolKind {
    match kind {
        DeclarationKind::Function => SymbolKind::Function,
        DeclarationKind::Method => SymbolKind::Method,
        DeclarationKind::Type | DeclarationKind::Struct | DeclarationKind::Enum
        | DeclarationKind::Interface | DeclarationKind::Trait => SymbolKind::Type,
        DeclarationKind::Const => SymbolKind::Const,
    }
}

/// Check that all required symbols exist in the codebase.
///
/// Uses AST-backed analysis for supported languages. Files with unsupported
/// extensions will cause an explicit failure for any symbols required in them.
pub fn detect_missing_symbols<P: AsRef<Path>>(
    analysis_ctx: &AnalysisContext,
    files: &[P],
    symbols: &[RequiredSymbol],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    if symbols.is_empty() {
        return Ok(result);
    }

    let base = analysis_ctx.base_dir();

    // Collect the set of files we actually need to analyze
    let required_files: HashSet<&str> = symbols.iter().map(|s| s.file.as_str()).collect();

    // Track which required files have unsupported extensions
    let mut unsupported_files: HashSet<String> = HashSet::new();

    // Build a map of found symbols by file (only for required files)
    let mut found_symbols: HashMap<String, Vec<SymbolInfo>> = HashMap::new();

    // Sort files for deterministic processing
    let mut sorted_files: Vec<_> = files.iter().collect();
    sorted_files.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));

    for file in sorted_files {
        let path = file.as_ref();
        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Skip files that aren't needed
        if !required_files.contains(rel_path.as_str()) {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Check if we have an analyzer for this extension
        if get_analyzer(ext).is_none() {
            unsupported_files.insert(rel_path.clone());
            continue;
        }

        // Use AST-backed analysis
        match analysis_ctx.analyze_file(path) {
            Ok(facts) => {
                let syms = extract_symbols_from_facts(&facts);
                found_symbols.insert(rel_path, syms);
                result.scanned += 1;
            }
            Err(e) => {
                // Parse error - emit a finding
                result.add_violation(Violation {
                    rule: ViolationRule::MissingSymbol,
                    message: format!("failed to parse file for symbol extraction: {}", e),
                    file: rel_path,
                    line: 0,
                    severity: Severity::Error,
                });
            }
        }
    }

    // Check each required symbol
    let mut violations: Vec<Violation> = Vec::new();

    for req in symbols {
        // Check if the file has an unsupported extension
        if unsupported_files.contains(&req.file) {
            violations.push(Violation {
                rule: ViolationRule::MissingSymbol,
                message: format!(
                    "cannot verify {} {:?}: no analyzer for file extension",
                    req.kind, req.name
                ),
                file: req.file.clone(),
                line: 0,
                severity: Severity::Critical,
            });
            continue;
        }

        let found = found_symbols
            .get(&req.file)
            .map(|syms| {
                syms.iter()
                    .any(|s| s.name == req.name && s.kind == req.kind)
            })
            .unwrap_or(false);

        if !found {
            violations.push(Violation {
                rule: ViolationRule::MissingSymbol,
                message: format!("required {} {:?} not found", req.kind, req.name),
                file: req.file.clone(),
                line: 0,
                severity: Severity::Critical,
            });
        }
    }

    // Sort violations for deterministic output
    violations.sort_by(|a, b| {
        (&a.file, a.line, &a.message).cmp(&(&b.file, b.line, &b.message))
    });

    for v in violations {
        result.add_violation(v);
    }

    Ok(result)
}

/// Extract symbols from FileFacts.
fn extract_symbols_from_facts(facts: &FileFacts) -> Vec<SymbolInfo> {
    facts
        .declarations
        .iter()
        .map(|decl| SymbolInfo {
            name: decl.name.clone(),
            kind: declaration_kind_to_symbol_kind(decl.kind),
            file: facts.path.clone(),
            line: decl.span.start_line,
        })
        .collect()
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

    // Create an analysis context for this check
    let analysis_ctx = AnalysisContext::new(base);

    // Build a map of found test functions by file
    let mut found_tests: HashMap<String, Vec<String>> = HashMap::new();

    // Sort files for deterministic processing
    let mut sorted_files: Vec<_> = files.iter().collect();
    sorted_files.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));

    // Only parse test files
    for file in sorted_files {
        let path = file.as_ref();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Only parse Go test files for now
        if !file_name.ends_with("_test.go") {
            continue;
        }

        let rel_path = path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Use AST-backed analysis
        if let Ok(facts) = analysis_ctx.analyze_file(path) {
            let test_names: Vec<String> = facts
                .declarations
                .iter()
                .filter(|d| {
                    d.kind == DeclarationKind::Function && d.name.starts_with("Test")
                })
                .map(|d| d.name.clone())
                .collect();

            found_tests.insert(rel_path, test_names);
        }
    }

    // Check each required test
    let mut violations: Vec<Violation> = Vec::new();

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
            violations.push(Violation {
                rule: ViolationRule::MissingTest,
                message: format!("required test {:?} not found", req.name),
                file,
                line: 0,
                severity: Severity::Warning,
            });
        }
    }

    // Sort violations for deterministic output
    violations.sort_by(|a, b| {
        (&a.file, a.line, &a.message).cmp(&(&b.file, b.line, &b.message))
    });

    for v in violations {
        result.add_violation(v);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_symbols_from_facts() {
        // Initialize analyzers
        crate::analysis::register_analyzers();

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

        let analysis_ctx = AnalysisContext::new(temp.path());
        let facts = analysis_ctx.analyze_file(&file_path).unwrap();
        let symbols = extract_symbols_from_facts(&facts);

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
        crate::analysis::register_analyzers();

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

        let analysis_ctx = AnalysisContext::new(temp.path());
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

        let result = detect_missing_symbols(&analysis_ctx, &[&file_path], &symbols).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("Handler"));
    }

    #[test]
    fn test_unsupported_extension_fails() {
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.xyz");
        std::fs::write(&file_path, "some content").unwrap();

        let analysis_ctx = AnalysisContext::new(temp.path());
        let symbols = vec![RequiredSymbol {
            name: "SomeFunc".to_string(),
            kind: SymbolKind::Function,
            file: "main.xyz".to_string(),
        }];

        let result = detect_missing_symbols(&analysis_ctx, &[&file_path], &symbols).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0]
            .message
            .contains("no analyzer for file extension"));
        assert_eq!(result.violations[0].severity, Severity::Critical);
    }
}

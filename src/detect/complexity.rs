//! Detection of functions with low cyclomatic complexity.
//!
//! This module uses AST-backed analysis via the AnalysisContext to calculate
//! cyclomatic complexity from control-flow facts extracted by tree-sitter.
//!
//! Cyclomatic complexity is calculated as:
//! - Start at 1
//! - Add 1 for each: if, for, while, case, &&, ||, ?, catch

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::analysis::{get_analyzer, AnalysisContext, DeclarationKind, FileFacts};
use crate::contract::ComplexityRequirement;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Complexity information for a function.
#[derive(Debug, Clone)]
struct FuncComplexity {
    name: String,
    complexity: i32,
    #[allow(dead_code)]
    file: String,
    line: usize,
}

/// Check that functions meet minimum complexity requirements.
///
/// Uses AST-backed analysis for supported languages. Files with unsupported
/// extensions will cause an explicit failure for any complexity checks in them.
pub fn detect_low_complexity<P: AsRef<Path>>(
    analysis_ctx: &AnalysisContext,
    files: &[P],
    requirements: &[ComplexityRequirement],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    if requirements.is_empty() {
        return Ok(result);
    }

    let base = analysis_ctx.base_dir();

    // Collect the set of files we need to analyze (only those with explicit requirements)
    let required_files: HashSet<&str> = requirements
        .iter()
        .filter_map(|req| req.file.as_deref())
        .collect();

    // Check if any requirement doesn't specify a file (needs to scan all files)
    let needs_full_scan = requirements.iter().any(|req| req.file.is_none());

    // Track which required files have unsupported extensions
    let mut unsupported_files: HashSet<String> = HashSet::new();

    // Build a map of function complexities by file
    let mut funcs_by_file: HashMap<String, Vec<FuncComplexity>> = HashMap::new();

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

        // Skip files that aren't needed (unless we need a full scan)
        if !needs_full_scan && !required_files.contains(rel_path.as_str()) {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Check if we have an analyzer for this extension
        if get_analyzer(ext).is_none() {
            if required_files.contains(rel_path.as_str()) {
                unsupported_files.insert(rel_path.clone());
            }
            continue;
        }

        // Use AST-backed analysis
        match analysis_ctx.analyze_file(path) {
            Ok(facts) => {
                let funcs = extract_complexities_from_facts(&facts);
                funcs_by_file.insert(rel_path, funcs);
                result.scanned += 1;
            }
            Err(e) => {
                // Parse error - emit a finding
                result.add_violation(Violation {
                    rule: ViolationRule::LowComplexity,
                    message: format!("failed to parse file for complexity analysis: {}", e),
                    file: rel_path,
                    line: 0,
                    severity: Severity::Error,
                });
            }
        }
    }

    // Check each complexity requirement
    let mut violations: Vec<Violation> = Vec::new();

    for req in requirements {
        // Check if the file has an unsupported extension
        if let Some(ref file) = req.file {
            if unsupported_files.contains(file) {
                violations.push(Violation {
                    rule: ViolationRule::LowComplexity,
                    message: format!(
                        "cannot verify complexity for {:?}: no analyzer for file extension",
                        req.symbol
                    ),
                    file: file.clone(),
                    line: 0,
                    severity: Severity::Error,
                });
                continue;
            }
        }

        let (found, actual_complexity, line) = if let Some(ref file) = req.file {
            // Look in specific file
            funcs_by_file
                .get(file)
                .and_then(|funcs| funcs.iter().find(|f| f.name == req.symbol))
                .map(|f| (true, f.complexity, f.line))
                .unwrap_or((false, 0, 0))
        } else {
            // Look in any file
            funcs_by_file
                .values()
                .flatten()
                .find(|f| f.name == req.symbol)
                .map(|f| (true, f.complexity, f.line))
                .unwrap_or((false, 0, 0))
        };

        if !found {
            let file = req.file.clone().unwrap_or_else(|| "(any file)".to_string());
            violations.push(Violation {
                rule: ViolationRule::LowComplexity,
                message: format!("symbol {:?} not found for complexity check", req.symbol),
                file,
                line: 0,
                severity: Severity::Error,
            });
            continue;
        }

        if actual_complexity < req.min_complexity {
            let file = req
                .file
                .clone()
                .unwrap_or_else(|| "(found in codebase)".to_string());
            violations.push(Violation {
                rule: ViolationRule::LowComplexity,
                message: format!(
                    "symbol {:?} has complexity {}, minimum required is {}",
                    req.symbol, actual_complexity, req.min_complexity
                ),
                file,
                line,
                severity: Severity::Error,
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

/// Extract complexity information from FileFacts.
fn extract_complexities_from_facts(facts: &FileFacts) -> Vec<FuncComplexity> {
    facts
        .declarations
        .iter()
        .filter(|d| d.kind == DeclarationKind::Function || d.kind == DeclarationKind::Method)
        .filter_map(|decl| {
            decl.body.as_ref().map(|body| FuncComplexity {
                name: decl.name.clone(),
                complexity: body.control_flow.cyclomatic_complexity(),
                file: facts.path.clone(),
                line: decl.span.start_line,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_go_complexity() {
        // Initialize analyzers
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        std::fs::write(
            &file_path,
            r#"
package main

func process(x int) int {
    if x > 0 {
        for i := 0; i < x; i++ {
            if i%2 == 0 && i > 5 {
                return i
            }
        }
    }
    return 0
}
"#,
        )
        .unwrap();

        let analysis_ctx = AnalysisContext::new(temp.path());
        let facts = analysis_ctx.analyze_file(&file_path).unwrap();
        let funcs = extract_complexities_from_facts(&facts);

        assert_eq!(funcs.len(), 1);
        // 1 (base) + 2 (if) + 1 (for) + 1 (&&) = 5
        assert_eq!(funcs[0].complexity, 5);
    }

    #[test]
    fn test_detect_low_complexity() {
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        std::fs::write(
            &file_path,
            r#"
package main

func simple() {
    return
}

func complex(x int) int {
    if x > 0 {
        for i := 0; i < x; i++ {
            if i%2 == 0 {
                return i
            }
        }
    }
    return 0
}
"#,
        )
        .unwrap();

        let analysis_ctx = AnalysisContext::new(temp.path());
        let requirements = vec![
            ComplexityRequirement {
                symbol: "simple".to_string(),
                file: Some("main.go".to_string()),
                min_complexity: 3,
            },
            ComplexityRequirement {
                symbol: "complex".to_string(),
                file: Some("main.go".to_string()),
                min_complexity: 3,
            },
        ];

        let result = detect_low_complexity(&analysis_ctx, &[&file_path], &requirements).unwrap();
        // simple has complexity 1, required 3 -> violation
        // complex has complexity 4, required 3 -> ok
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("simple"));
    }

    #[test]
    fn test_unsupported_extension_fails() {
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.xyz");
        std::fs::write(&file_path, "some content").unwrap();

        let analysis_ctx = AnalysisContext::new(temp.path());
        let requirements = vec![ComplexityRequirement {
            symbol: "SomeFunc".to_string(),
            file: Some("main.xyz".to_string()),
            min_complexity: 5,
        }];

        let result = detect_low_complexity(&analysis_ctx, &[&file_path], &requirements).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0]
            .message
            .contains("no analyzer for file extension"));
    }

    #[test]
    fn test_complexity_symbol_not_found() {
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
        let requirements = vec![ComplexityRequirement {
            symbol: "nonexistent".to_string(),
            file: Some("main.go".to_string()),
            min_complexity: 5,
        }];

        let result = detect_low_complexity(&analysis_ctx, &[&file_path], &requirements).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("not found"));
    }
}

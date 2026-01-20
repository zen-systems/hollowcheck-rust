//! Detection of functions with low cyclomatic complexity.
//!
//! Cyclomatic complexity is calculated as:
//! - Start at 1
//! - Add 1 for each: if, for, while, case, &&, ||, ?, catch
//!
//! When the tree-sitter feature is enabled, AST-based complexity calculation
//! is used for more accurate results. Otherwise, regex-based detection is used.

use crate::contract::ComplexityRequirement;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Complexity information for a function.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FuncComplexity {
    name: String,
    complexity: i32,
    file: String,
    line: usize,
}

/// Calculate complexities using tree-sitter when available.
///
/// Returns None if tree-sitter is not available or fails, allowing fallback to regex.
#[cfg(feature = "tree-sitter")]
fn calculate_complexities_treesitter(file_path: &Path, ext: &str) -> Option<Vec<FuncComplexity>> {
    use crate::parser;

    // Get the extension with dot prefix for the registry
    let ext_with_dot = format!(".{}", ext);
    let ts_parser = parser::for_extension(&ext_with_dot)?;

    let source = fs::read(file_path).ok()?;
    let file_str = file_path.to_string_lossy().to_string();

    // First, get all symbols to find function names and lines
    let symbols = ts_parser.parse_symbols(&source).ok()?;

    let mut funcs = Vec::new();
    for symbol in symbols {
        // Only calculate complexity for functions and methods
        if symbol.kind != "function" && symbol.kind != "method" {
            continue;
        }

        // Use tree-sitter to calculate complexity
        let complexity = ts_parser.complexity(&source, &symbol.name).ok()?;

        funcs.push(FuncComplexity {
            name: symbol.name,
            complexity,
            file: file_str.clone(),
            line: symbol.line,
        });
    }

    Some(funcs)
}

/// Stub for when tree-sitter feature is not enabled.
#[cfg(not(feature = "tree-sitter"))]
fn calculate_complexities_treesitter(_file_path: &Path, _ext: &str) -> Option<Vec<FuncComplexity>> {
    None
}

/// Check that functions meet minimum complexity requirements.
///
/// Optimized to only parse files that are explicitly specified in requirements,
/// rather than parsing all files in the codebase.
pub fn detect_low_complexity<P1: AsRef<Path>, P2: AsRef<Path>>(
    base_dir: P1,
    files: &[P2],
    requirements: &[ComplexityRequirement],
) -> anyhow::Result<DetectionResult> {
    use std::collections::HashSet;

    let mut result = DetectionResult::new();

    if requirements.is_empty() {
        return Ok(result);
    }

    let base = base_dir.as_ref();

    // Collect the set of files we need to parse (only those with explicit requirements)
    let required_files: HashSet<&str> = requirements
        .iter()
        .filter_map(|req| req.file.as_deref())
        .collect();

    // Check if any requirement doesn't specify a file (needs to scan all files)
    let needs_full_scan = requirements.iter().any(|req| req.file.is_none());

    // Build a map of function complexities by file
    let mut funcs_by_file: HashMap<String, Vec<FuncComplexity>> = HashMap::new();

    for file in files {
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

        // Try tree-sitter first, fall back to regex-based extraction
        let funcs = if let Some(ts_funcs) = calculate_complexities_treesitter(path, ext) {
            ts_funcs
        } else {
            // Fall back to regex-based complexity calculation
            match ext {
                "go" => calculate_complexities_go(path)?,
                "rs" => calculate_complexities_rust(path)?,
                "py" => calculate_complexities_python(path)?,
                "js" | "ts" | "jsx" | "tsx" => calculate_complexities_js(path)?,
                "java" => calculate_complexities_java(path)?,
                _ => continue,
            }
        };

        funcs_by_file.insert(rel_path, funcs);
        result.scanned += 1;
    }

    // Check each complexity requirement
    for req in requirements {
        let (found, actual_complexity) = if let Some(ref file) = req.file {
            // Look in specific file
            funcs_by_file
                .get(file)
                .and_then(|funcs| funcs.iter().find(|f| f.name == req.symbol))
                .map(|f| (true, f.complexity))
                .unwrap_or((false, 0))
        } else {
            // Look in any file
            funcs_by_file
                .values()
                .flatten()
                .find(|f| f.name == req.symbol)
                .map(|f| (true, f.complexity))
                .unwrap_or((false, 0))
        };

        if !found {
            let file = req.file.clone().unwrap_or_else(|| "(any file)".to_string());
            result.add_violation(Violation {
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
            result.add_violation(Violation {
                rule: ViolationRule::LowComplexity,
                message: format!(
                    "symbol {:?} has complexity {}, minimum required is {}",
                    req.symbol, actual_complexity, req.min_complexity
                ),
                file,
                line: 0,
                severity: Severity::Error,
            });
        }
    }

    Ok(result)
}

/// Calculate cyclomatic complexity for all functions in a Go file.
fn calculate_complexities_go(file_path: &Path) -> anyhow::Result<Vec<FuncComplexity>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    // Find all functions and their bodies
    let func_re = Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?(\w+)\s*\([^)]*\)")?;

    let mut funcs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                // Find the function body by counting braces
                let body = extract_function_body(&lines, line_num);
                let complexity = calculate_go_complexity(&body);

                funcs.push(FuncComplexity {
                    name: name.as_str().to_string(),
                    complexity,
                    file: file_str.clone(),
                    line: line_num + 1,
                });
            }
        }
    }

    Ok(funcs)
}

/// Extract the body of a function starting from a given line.
fn extract_function_body(lines: &[&str], start_line: usize) -> String {
    let mut body = String::new();
    let mut brace_count = 0;
    let mut found_first_brace = false;

    for line in lines.iter().skip(start_line) {
        for ch in line.chars() {
            if ch == '{' {
                brace_count += 1;
                found_first_brace = true;
            } else if ch == '}' {
                brace_count -= 1;
            }
        }

        body.push_str(line);
        body.push('\n');

        if found_first_brace && brace_count == 0 {
            break;
        }
    }

    body
}

/// Calculate cyclomatic complexity for Go code.
fn calculate_go_complexity(body: &str) -> i32 {
    let mut complexity = 1;

    // Count decision points
    // if statements
    let if_re = Regex::new(r"\bif\b").unwrap();
    complexity += if_re.find_iter(body).count() as i32;

    // for loops (including range)
    let for_re = Regex::new(r"\bfor\b").unwrap();
    complexity += for_re.find_iter(body).count() as i32;

    // switch cases (excluding default)
    let case_re = Regex::new(r"\bcase\b").unwrap();
    complexity += case_re.find_iter(body).count() as i32;

    // Logical operators
    let and_re = Regex::new(r"&&").unwrap();
    complexity += and_re.find_iter(body).count() as i32;

    let or_re = Regex::new(r"\|\|").unwrap();
    complexity += or_re.find_iter(body).count() as i32;

    complexity
}

/// Calculate cyclomatic complexity for all functions in a Rust file.
fn calculate_complexities_rust(file_path: &Path) -> anyhow::Result<Vec<FuncComplexity>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    let func_re = Regex::new(r"(?m)^\s*(?:pub\s+)?fn\s+(\w+)")?;

    let mut funcs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                let body = extract_function_body(&lines, line_num);
                let complexity = calculate_rust_complexity(&body);

                funcs.push(FuncComplexity {
                    name: name.as_str().to_string(),
                    complexity,
                    file: file_str.clone(),
                    line: line_num + 1,
                });
            }
        }
    }

    Ok(funcs)
}

/// Calculate cyclomatic complexity for Rust code.
fn calculate_rust_complexity(body: &str) -> i32 {
    let mut complexity = 1;

    let if_re = Regex::new(r"\bif\b").unwrap();
    complexity += if_re.find_iter(body).count() as i32;

    let for_re = Regex::new(r"\bfor\b").unwrap();
    complexity += for_re.find_iter(body).count() as i32;

    let while_re = Regex::new(r"\bwhile\b").unwrap();
    complexity += while_re.find_iter(body).count() as i32;

    let loop_re = Regex::new(r"\bloop\b").unwrap();
    complexity += loop_re.find_iter(body).count() as i32;

    let match_arm_re = Regex::new(r"=>").unwrap();
    complexity += match_arm_re.find_iter(body).count() as i32;

    let and_re = Regex::new(r"&&").unwrap();
    complexity += and_re.find_iter(body).count() as i32;

    let or_re = Regex::new(r"\|\|").unwrap();
    complexity += or_re.find_iter(body).count() as i32;

    complexity
}

/// Calculate cyclomatic complexity for all functions in a Python file.
fn calculate_complexities_python(file_path: &Path) -> anyhow::Result<Vec<FuncComplexity>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    let func_re = Regex::new(r"(?m)^(?:\s*)def\s+(\w+)\s*\(")?;

    let mut funcs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                // For Python, extract body based on indentation
                let body = extract_python_function_body(&lines, line_num);
                let complexity = calculate_python_complexity(&body);

                funcs.push(FuncComplexity {
                    name: name.as_str().to_string(),
                    complexity,
                    file: file_str.clone(),
                    line: line_num + 1,
                });
            }
        }
    }

    Ok(funcs)
}

/// Extract a Python function body based on indentation.
fn extract_python_function_body(lines: &[&str], start_line: usize) -> String {
    let mut body = String::new();

    if start_line >= lines.len() {
        return body;
    }

    // Get the indentation of the def line
    let def_line = lines[start_line];
    let def_indent = def_line.len() - def_line.trim_start().len();

    body.push_str(def_line);
    body.push('\n');

    for line in lines.iter().skip(start_line + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            body.push('\n');
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        if indent <= def_indent {
            // Back to same or lower indentation, function ended
            break;
        }

        body.push_str(line);
        body.push('\n');
    }

    body
}

/// Calculate cyclomatic complexity for Python code.
fn calculate_python_complexity(body: &str) -> i32 {
    let mut complexity = 1;

    let if_re = Regex::new(r"\bif\b").unwrap();
    complexity += if_re.find_iter(body).count() as i32;

    let elif_re = Regex::new(r"\belif\b").unwrap();
    complexity += elif_re.find_iter(body).count() as i32;

    let for_re = Regex::new(r"\bfor\b").unwrap();
    complexity += for_re.find_iter(body).count() as i32;

    let while_re = Regex::new(r"\bwhile\b").unwrap();
    complexity += while_re.find_iter(body).count() as i32;

    let except_re = Regex::new(r"\bexcept\b").unwrap();
    complexity += except_re.find_iter(body).count() as i32;

    let and_re = Regex::new(r"\band\b").unwrap();
    complexity += and_re.find_iter(body).count() as i32;

    let or_re = Regex::new(r"\bor\b").unwrap();
    complexity += or_re.find_iter(body).count() as i32;

    complexity
}

/// Calculate cyclomatic complexity for all functions in a JS/TS file.
fn calculate_complexities_js(file_path: &Path) -> anyhow::Result<Vec<FuncComplexity>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    // Match function declarations
    let func_re = Regex::new(r"(?m)(?:^|\s)(?:export\s+)?(?:async\s+)?function\s+(\w+)")?;

    let mut funcs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if let Some(caps) = func_re.captures(line) {
            if let Some(name) = caps.get(1) {
                let body = extract_function_body(&lines, line_num);
                let complexity = calculate_js_complexity(&body);

                funcs.push(FuncComplexity {
                    name: name.as_str().to_string(),
                    complexity,
                    file: file_str.clone(),
                    line: line_num + 1,
                });
            }
        }
    }

    Ok(funcs)
}

/// Calculate cyclomatic complexity for JavaScript/TypeScript code.
fn calculate_js_complexity(body: &str) -> i32 {
    let mut complexity = 1;

    let if_re = Regex::new(r"\bif\b").unwrap();
    complexity += if_re.find_iter(body).count() as i32;

    let for_re = Regex::new(r"\bfor\b").unwrap();
    complexity += for_re.find_iter(body).count() as i32;

    let while_re = Regex::new(r"\bwhile\b").unwrap();
    complexity += while_re.find_iter(body).count() as i32;

    let case_re = Regex::new(r"\bcase\b").unwrap();
    complexity += case_re.find_iter(body).count() as i32;

    let catch_re = Regex::new(r"\bcatch\b").unwrap();
    complexity += catch_re.find_iter(body).count() as i32;

    let ternary_re = Regex::new(r"\?").unwrap();
    // Exclude TypeScript optional chaining ?.
    let optional_chain_re = Regex::new(r"\?\.\w").unwrap();
    complexity += ternary_re.find_iter(body).count() as i32;
    complexity -= optional_chain_re.find_iter(body).count() as i32;

    let and_re = Regex::new(r"&&").unwrap();
    complexity += and_re.find_iter(body).count() as i32;

    let or_re = Regex::new(r"\|\|").unwrap();
    complexity += or_re.find_iter(body).count() as i32;

    complexity.max(1)
}

/// Calculate cyclomatic complexity for all methods in a Java file.
fn calculate_complexities_java(file_path: &Path) -> anyhow::Result<Vec<FuncComplexity>> {
    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    // Match method declarations (simplified - doesn't handle all cases perfectly)
    let method_re = Regex::new(
        r"(?m)^\s*(?:public|private|protected)?\s*(?:static)?\s*(?:\w+(?:<[^>]+>)?)\s+(\w+)\s*\(",
    )?;

    let mut funcs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        if let Some(caps) = method_re.captures(line) {
            if let Some(name) = caps.get(1) {
                let name_str = name.as_str();
                // Skip class declarations (constructor names match class names, etc.)
                if name_str.chars().next().is_some_and(|c| c.is_uppercase()) {
                    continue;
                }
                let body = extract_function_body(&lines, line_num);
                let complexity = calculate_java_complexity(&body);

                funcs.push(FuncComplexity {
                    name: name_str.to_string(),
                    complexity,
                    file: file_str.clone(),
                    line: line_num + 1,
                });
            }
        }
    }

    Ok(funcs)
}

/// Calculate cyclomatic complexity for Java code.
fn calculate_java_complexity(body: &str) -> i32 {
    let mut complexity = 1;

    let if_re = Regex::new(r"\bif\b").unwrap();
    complexity += if_re.find_iter(body).count() as i32;

    let for_re = Regex::new(r"\bfor\b").unwrap();
    complexity += for_re.find_iter(body).count() as i32;

    let while_re = Regex::new(r"\bwhile\b").unwrap();
    complexity += while_re.find_iter(body).count() as i32;

    let case_re = Regex::new(r"\bcase\b").unwrap();
    complexity += case_re.find_iter(body).count() as i32;

    let catch_re = Regex::new(r"\bcatch\b").unwrap();
    complexity += catch_re.find_iter(body).count() as i32;

    let ternary_re = Regex::new(r"\?").unwrap();
    complexity += ternary_re.find_iter(body).count() as i32;

    let and_re = Regex::new(r"&&").unwrap();
    complexity += and_re.find_iter(body).count() as i32;

    let or_re = Regex::new(r"\|\|").unwrap();
    complexity += or_re.find_iter(body).count() as i32;

    complexity.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_go_complexity() {
        let body = r#"
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
"#;
        // 1 (base) + 2 (if) + 1 (for) + 1 (&&) = 5
        assert_eq!(calculate_go_complexity(body), 5);
    }

    #[test]
    fn test_detect_low_complexity() {
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

        let result = detect_low_complexity(temp.path(), &[&file_path], &requirements).unwrap();
        // simple has complexity 1, required 3 -> violation
        // complex has complexity 4, required 3 -> ok
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("simple"));
    }
}

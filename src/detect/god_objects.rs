//! Detection of god objects, god files, and god functions.
//!
//! This module identifies architectural code smells where components
//! have grown too large or complex:
//! - God files: Too many lines or functions
//! - God functions: Too many lines or excessive complexity
//! - God classes: Too many methods

use std::collections::HashMap;
use std::path::Path;

use crate::parser;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Configuration for god object detection.
#[derive(Debug, Clone)]
pub struct GodObjectConfig {
    /// Maximum lines per file before flagging (default: 1000, strict: 500)
    pub max_file_lines: usize,
    /// Maximum lines per function before flagging (default: 100, strict: 50)
    pub max_function_lines: usize,
    /// Maximum cyclomatic complexity per function (default: 20, strict: 15)
    pub max_function_complexity: usize,
    /// Maximum functions per file (default: 30, strict: 20)
    pub max_functions_per_file: usize,
    /// Maximum methods per class (default: 20, strict: 15)
    pub max_class_methods: usize,
}

impl Default for GodObjectConfig {
    fn default() -> Self {
        Self {
            max_file_lines: 1000,
            max_function_lines: 100,
            max_function_complexity: 20,
            max_functions_per_file: 30,
            max_class_methods: 20,
        }
    }
}

impl GodObjectConfig {
    /// Return strict thresholds optimized for AI-generated code detection.
    /// These are more aggressive and catch more issues.
    pub fn strict() -> Self {
        Self {
            max_file_lines: 500,
            max_function_lines: 50,
            max_function_complexity: 15,
            max_functions_per_file: 20,
            max_class_methods: 15,
        }
    }

    /// Return relaxed thresholds for large, mature codebases.
    pub fn relaxed() -> Self {
        Self {
            max_file_lines: 2000,
            max_function_lines: 200,
            max_function_complexity: 30,
            max_functions_per_file: 50,
            max_class_methods: 30,
        }
    }
}

/// Detect god objects in the given files.
pub fn detect_god_objects<P: AsRef<Path>>(
    files: &[P],
    config: &GodObjectConfig,
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();

    for file in files {
        let file_path = file.as_ref();
        let file_violations = check_file(file_path, config)?;
        result.violations.extend(file_violations);
        result.scanned += 1;
    }

    Ok(result)
}

/// Check a single file for god object issues.
fn check_file(file_path: &Path, config: &GodObjectConfig) -> anyhow::Result<Vec<Violation>> {
    let mut violations = Vec::new();
    let file_str = file_path.to_string_lossy().to_string();

    // Read file content ONCE and reuse
    let content = std::fs::read(file_path)?;
    let content_str = String::from_utf8_lossy(&content);
    let lines: Vec<&str> = content_str.lines().collect();
    let line_count = lines.len();

    // Check file line count first (cheap check)
    let exceeds_file_lines = line_count > config.max_file_lines;
    if exceeds_file_lines {
        violations.push(Violation {
            rule: ViolationRule::GodFile,
            message: format!(
                "file has {} lines, exceeds maximum of {}",
                line_count, config.max_file_lines
            ),
            file: file_str.clone(),
            line: 1,
            severity: Severity::Warning,
        });
    }

    // EARLY EXIT: If file is small enough, skip expensive tree-sitter parsing
    // A file with N lines can have at most N one-line functions
    // So only skip if: file is too small for god functions AND too few potential functions
    // This is conservative - we skip only when the file clearly can't trigger any violations
    let min_lines_for_function_count = config.max_functions_per_file; // At least 1 line per function
    let skip_parsing = line_count <= config.max_function_lines
        && line_count < min_lines_for_function_count;

    if skip_parsing {
        return Ok(violations);
    }

    // Try to get parser for this file
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e));

    let Some(ext) = extension else {
        return Ok(violations);
    };

    let Some(file_parser) = parser::for_extension(&ext) else {
        return Ok(violations);
    };

    // Parse symbols with complexity in ONE pass (optimized)
    let symbols_with_complexity = file_parser.parse_symbols_with_complexity(&content)?;

    // Count functions per file
    let function_count = symbols_with_complexity
        .iter()
        .filter(|s| s.symbol.kind == "function")
        .count();
    if function_count > config.max_functions_per_file {
        violations.push(Violation {
            rule: ViolationRule::GodFile,
            message: format!(
                "file has {} functions, exceeds maximum of {}",
                function_count, config.max_functions_per_file
            ),
            file: file_str.clone(),
            line: 1,
            severity: Severity::Warning,
        });
    }

    // Group methods by class (for class method counting)
    let mut class_methods: HashMap<String, Vec<&str>> = HashMap::new();
    for swc in &symbols_with_complexity {
        if swc.symbol.kind == "method" {
            // Try to extract class name from method name (e.g., "ClassName.methodName")
            if let Some(class_name) = extract_class_from_method(&swc.symbol.name) {
                class_methods
                    .entry(class_name.to_string())
                    .or_default()
                    .push(&swc.symbol.name);
            }
        }
    }

    // Check class method counts
    for (class_name, methods) in &class_methods {
        if methods.len() > config.max_class_methods {
            // Find the line of the first method for this class
            let first_method_line = symbols_with_complexity
                .iter()
                .find(|s| s.symbol.kind == "method" && s.symbol.name.starts_with(class_name))
                .map(|s| s.symbol.line)
                .unwrap_or(1);

            violations.push(Violation {
                rule: ViolationRule::GodClass,
                message: format!(
                    "class '{}' has {} methods, exceeds maximum of {}",
                    class_name,
                    methods.len(),
                    config.max_class_methods
                ),
                file: file_str.clone(),
                line: first_method_line,
                severity: Severity::Warning,
            });
        }
    }

    // Build a simple symbol list for line estimation
    let symbols: Vec<_> = symbols_with_complexity
        .iter()
        .map(|swc| &swc.symbol)
        .cloned()
        .collect();

    // Check individual function complexity and length
    for swc in &symbols_with_complexity {
        if swc.symbol.kind == "function" || swc.symbol.kind == "method" {
            // Estimate function length first (cheap operation)
            let func_lines = estimate_function_lines_fast(&lines, swc.symbol.line, &symbols);
            if func_lines > config.max_function_lines {
                violations.push(Violation {
                    rule: ViolationRule::GodFunction,
                    message: format!(
                        "function '{}' has ~{} lines, exceeds maximum of {}",
                        swc.symbol.name, func_lines, config.max_function_lines
                    ),
                    file: file_str.clone(),
                    line: swc.symbol.line,
                    severity: Severity::Warning,
                });
            }

            // Only check complexity if function is large enough to matter
            // (small functions can't have high complexity)
            // A function needs at least ~10 lines to potentially exceed complexity 15-20
            if func_lines > 10 {
                if let Some(complexity) = swc.complexity {
                    if complexity > config.max_function_complexity as i32 {
                        violations.push(Violation {
                            rule: ViolationRule::GodFunction,
                            message: format!(
                                "function '{}' has complexity {}, exceeds maximum of {}",
                                swc.symbol.name, complexity, config.max_function_complexity
                            ),
                            file: file_str.clone(),
                            line: swc.symbol.line,
                            severity: Severity::Warning,
                        });
                    }
                }
            }
        }
    }

    Ok(violations)
}

/// Extract class name from a method name like "ClassName.methodName" or "ClassName::methodName".
fn extract_class_from_method(method_name: &str) -> Option<&str> {
    // Try common separators: . :: ->
    if let Some(pos) = method_name.find('.') {
        return Some(&method_name[..pos]);
    }
    if let Some(pos) = method_name.find("::") {
        return Some(&method_name[..pos]);
    }
    if let Some(pos) = method_name.find("->") {
        return Some(&method_name[..pos]);
    }
    None
}

/// Estimate the number of lines in a function by looking at the next symbol's line.
/// Uses borrowed slices for efficiency.
fn estimate_function_lines_fast(
    lines: &[&str],
    func_start: usize,
    symbols: &[parser::Symbol],
) -> usize {
    // Find the next symbol that starts after this function
    let next_symbol_line = symbols
        .iter()
        .filter(|s| s.line > func_start)
        .map(|s| s.line)
        .min();

    let func_end = match next_symbol_line {
        Some(line) => line.saturating_sub(1),
        None => lines.len(),
    };

    func_end.saturating_sub(func_start) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() {
        parser::init();
    }

    #[test]
    fn test_god_file_line_count() {
        setup();
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("big.go");

        // Create a file with many lines
        let content: String = (0..600).map(|i| format!("// line {}\n", i)).collect();
        std::fs::write(&file_path, format!("package main\n{}", content)).unwrap();

        let config = GodObjectConfig {
            max_file_lines: 500,
            ..Default::default()
        };

        let result = detect_god_objects(&[&file_path], &config).unwrap();
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == ViolationRule::GodFile && v.message.contains("lines")),
            "Expected god file violation for line count"
        );
    }

    #[test]
    fn test_god_file_function_count() {
        setup();
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("many_funcs.go");

        // Create a file with many functions
        let mut content = String::from("package main\n\n");
        for i in 0..25 {
            content.push_str(&format!("func func{}() {{}}\n\n", i));
        }
        std::fs::write(&file_path, content).unwrap();

        let config = GodObjectConfig {
            max_functions_per_file: 20,
            ..Default::default()
        };

        let result = detect_god_objects(&[&file_path], &config).unwrap();
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == ViolationRule::GodFile && v.message.contains("functions")),
            "Expected god file violation for function count"
        );
    }

    #[test]
    fn test_god_function_complexity() {
        setup();
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("complex.go");

        // Create a function with high complexity
        let content = r#"package main

func complexFunc(x int) int {
    if x > 0 {
        if x > 10 {
            if x > 20 {
                if x > 30 {
                    if x > 40 {
                        for i := 0; i < x; i++ {
                            if i%2 == 0 {
                                for j := 0; j < i; j++ {
                                    if j > 5 {
                                        switch j {
                                        case 1:
                                            return 1
                                        case 2:
                                            return 2
                                        case 3:
                                            return 3
                                        default:
                                            return 4
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    return 0
}
"#;
        std::fs::write(&file_path, content).unwrap();

        let config = GodObjectConfig {
            max_function_complexity: 5,
            ..Default::default()
        };

        let result = detect_god_objects(&[&file_path], &config).unwrap();
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == ViolationRule::GodFunction && v.message.contains("complexity")),
            "Expected god function violation for complexity"
        );
    }

    #[test]
    fn test_god_function_length() {
        setup();
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("long_func.go");

        // Create a function with many lines
        let mut func_body = String::new();
        for i in 0..60 {
            func_body.push_str(&format!("    x := {}\n", i));
        }

        let content = format!("package main\n\nfunc longFunc() {{\n{}}}\n", func_body);
        std::fs::write(&file_path, content).unwrap();

        let config = GodObjectConfig {
            max_function_lines: 50,
            ..Default::default()
        };

        let result = detect_god_objects(&[&file_path], &config).unwrap();
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.rule == ViolationRule::GodFunction && v.message.contains("lines")),
            "Expected god function violation for line count"
        );
    }

    #[test]
    fn test_clean_file_no_violations() {
        setup();
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("clean.go");

        let content = r#"package main

func main() {
    fmt.Println("Hello")
}

func helper() int {
    return 42
}
"#;
        std::fs::write(&file_path, content).unwrap();

        let config = GodObjectConfig::default();
        let result = detect_god_objects(&[&file_path], &config).unwrap();
        assert!(
            result.violations.is_empty(),
            "Expected no violations for clean file"
        );
    }

    #[test]
    fn test_extract_class_from_method() {
        assert_eq!(extract_class_from_method("Foo.bar"), Some("Foo"));
        assert_eq!(extract_class_from_method("Foo::bar"), Some("Foo"));
        assert_eq!(extract_class_from_method("Foo->bar"), Some("Foo"));
        assert_eq!(extract_class_from_method("bar"), None);
    }
}

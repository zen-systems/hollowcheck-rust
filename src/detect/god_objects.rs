//! Detection of god objects, god files, and god functions.
//!
//! This module identifies architectural code smells where components
//! have grown too large or complex:
//! - God files: Too many lines or functions
//! - God functions: Too many lines or excessive complexity
//! - God classes: Too many methods

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::parser;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Configuration for god object detection.
#[derive(Debug, Clone)]
pub struct GodObjectConfig {
    /// Maximum lines per file before flagging (default: 500)
    pub max_file_lines: usize,
    /// Maximum lines per function before flagging (default: 50)
    pub max_function_lines: usize,
    /// Maximum cyclomatic complexity per function (default: 15)
    pub max_function_complexity: usize,
    /// Maximum functions per file (default: 20)
    pub max_functions_per_file: usize,
    /// Maximum methods per class (default: 15)
    pub max_class_methods: usize,
}

impl Default for GodObjectConfig {
    fn default() -> Self {
        Self {
            max_file_lines: 500,
            max_function_lines: 50,
            max_function_complexity: 15,
            max_functions_per_file: 20,
            max_class_methods: 15,
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

    // Count total lines
    let line_count = count_lines(file_path)?;
    if line_count > config.max_file_lines {
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

    // Read file content
    let content = std::fs::read(file_path)?;

    // Parse symbols
    let symbols = file_parser.parse_symbols(&content)?;

    // Count functions per file
    let function_count = symbols.iter().filter(|s| s.kind == "function").count();
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
    for symbol in &symbols {
        if symbol.kind == "method" {
            // Try to extract class name from method name (e.g., "ClassName.methodName")
            if let Some(class_name) = extract_class_from_method(&symbol.name) {
                class_methods
                    .entry(class_name.to_string())
                    .or_default()
                    .push(&symbol.name);
            }
        }
    }

    // Check class method counts
    for (class_name, methods) in &class_methods {
        if methods.len() > config.max_class_methods {
            // Find the line of the first method for this class
            let first_method_line = symbols
                .iter()
                .find(|s| s.kind == "method" && s.name.starts_with(class_name))
                .map(|s| s.line)
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

    // Check individual function complexity and length
    let lines: Vec<String> = std::fs::read_to_string(file_path)?
        .lines()
        .map(String::from)
        .collect();

    for symbol in &symbols {
        if symbol.kind == "function" || symbol.kind == "method" {
            // Check complexity
            let complexity = file_parser.complexity(&content, &symbol.name)?;
            if complexity > config.max_function_complexity as i32 {
                violations.push(Violation {
                    rule: ViolationRule::GodFunction,
                    message: format!(
                        "function '{}' has complexity {}, exceeds maximum of {}",
                        symbol.name, complexity, config.max_function_complexity
                    ),
                    file: file_str.clone(),
                    line: symbol.line,
                    severity: Severity::Warning,
                });
            }

            // Estimate function length (lines until next function or end of file)
            let func_lines = estimate_function_lines(&lines, symbol.line, &symbols);
            if func_lines > config.max_function_lines {
                violations.push(Violation {
                    rule: ViolationRule::GodFunction,
                    message: format!(
                        "function '{}' has ~{} lines, exceeds maximum of {}",
                        symbol.name, func_lines, config.max_function_lines
                    ),
                    file: file_str.clone(),
                    line: symbol.line,
                    severity: Severity::Warning,
                });
            }
        }
    }

    Ok(violations)
}

/// Count the number of lines in a file.
fn count_lines(path: &Path) -> anyhow::Result<usize> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().count())
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
fn estimate_function_lines(
    lines: &[String],
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

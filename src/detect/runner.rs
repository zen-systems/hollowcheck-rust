//! Detection runner that orchestrates all checks.

use std::path::{Path, PathBuf};

use crate::contract::Contract;

use super::{
    collect_suppressions, detect_forbidden_patterns, detect_god_objects,
    detect_hallucinated_dependencies, detect_low_complexity, detect_missing_files,
    detect_missing_symbols, detect_missing_tests, detect_mock_data, filter_suppressed,
    DetectionResult, GodObjectConfig,
};

/// Executes all detection checks against a set of files.
pub struct Runner {
    base_dir: PathBuf,
    skip_registry_check: bool,
}

impl Runner {
    /// Create a new detection runner.
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            skip_registry_check: false,
        }
    }

    /// Set whether to skip registry checks for dependency verification.
    pub fn skip_registry_check(mut self, skip: bool) -> Self {
        self.skip_registry_check = skip;
        self
    }

    /// Run all detection checks defined in the contract.
    pub fn run(&self, files: &[PathBuf], contract: &Contract) -> anyhow::Result<DetectionResult> {
        let mut result = DetectionResult::new();

        // Collect suppressions from all files
        let suppression_map = collect_suppressions(files)?;
        let all_suppressions: Vec<_> = suppression_map.values().flatten().cloned().collect();

        // Check required files
        let file_result = detect_missing_files(&self.base_dir, &contract.required_files)?;
        result.merge(file_result);

        // Scan for forbidden patterns
        let pattern_result = detect_forbidden_patterns(files, &contract.forbidden_patterns)?;
        result.merge(pattern_result);

        // Scan for mock data signatures
        let mock_result = detect_mock_data(files, contract.mock_signatures.as_ref())?;
        result.merge(mock_result);

        // Check required symbols
        let symbol_result =
            detect_missing_symbols(&self.base_dir, files, &contract.required_symbols)?;
        result.merge(symbol_result);

        // Check complexity requirements
        let complexity_result = detect_low_complexity(&self.base_dir, files, &contract.complexity)?;
        result.merge(complexity_result);

        // Check required tests
        let test_result = detect_missing_tests(&self.base_dir, files, &contract.required_tests)?;
        result.merge(test_result);

        // Check for hallucinated dependencies (unless skipped)
        if !self.skip_registry_check {
            let dep_result = detect_hallucinated_dependencies(
                &self.base_dir,
                files,
                contract.dependency_verification.as_ref(),
            )?;
            result.merge(dep_result);
        }

        // Check for god objects (files, functions, classes that are too large)
        if let Some(god_cfg) = &contract.god_objects {
            if god_cfg.is_enabled() {
                let config = GodObjectConfig {
                    max_file_lines: god_cfg.max_file_lines.unwrap_or(500),
                    max_function_lines: god_cfg.max_function_lines.unwrap_or(50),
                    max_function_complexity: god_cfg.max_function_complexity.unwrap_or(15),
                    max_functions_per_file: god_cfg.max_functions_per_file.unwrap_or(20),
                    max_class_methods: god_cfg.max_class_methods.unwrap_or(15),
                };
                let god_result = detect_god_objects(files, &config)?;
                result.merge(god_result);
            }
        }

        // Apply suppressions - filter violations and track suppressed ones
        if !all_suppressions.is_empty() {
            let (active, suppressed) = filter_suppressed(result.violations, &all_suppressions);
            result.violations = active;
            result.suppressed = suppressed;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ForbiddenPattern, RequiredFile};
    use tempfile::TempDir;

    #[test]
    fn test_runner_basic() {
        let temp = TempDir::new().unwrap();
        let main_go = temp.path().join("main.go");
        let todo_marker = "TODO";
        std::fs::write(
            &main_go,
            format!(
                r#"
package main

// {}: implement this
func main() {{}}
"#,
                todo_marker
            ),
        )
        .unwrap();

        let contract = Contract {
            required_files: vec![RequiredFile {
                path: "main.go".to_string(),
                required: true,
            }],
            forbidden_patterns: vec![ForbiddenPattern {
                pattern: todo_marker.to_string(),
                description: Some("Remove TODOs".to_string()),
            }],
            ..Default::default()
        };

        let runner = Runner::new(temp.path());
        let result = runner.run(&[main_go], &contract).unwrap();

        // Should find the work marker violation but not missing file
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains(todo_marker));
    }

    #[test]
    fn test_runner_with_suppression() {
        let temp = TempDir::new().unwrap();
        let main_go = temp.path().join("main.go");
        let todo_marker = "TODO";
        std::fs::write(
            &main_go,
            format!(
                r#"
package main

// hollowcheck:ignore-next-line forbidden_pattern - Expected
// {}: implement this
func main() {{}}
"#,
                todo_marker
            ),
        )
        .unwrap();

        let contract = Contract {
            forbidden_patterns: vec![ForbiddenPattern {
                pattern: todo_marker.to_string(),
                description: None,
            }],
            ..Default::default()
        };

        let runner = Runner::new(temp.path());
        let result = runner.run(&[main_go], &contract).unwrap();

        // The work marker should be suppressed
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.suppressed.len(), 1);
    }
}

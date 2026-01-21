//! Detection runner that orchestrates all checks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::analysis::AnalysisContext;
use crate::contract::Contract;

use super::{
    collect_suppressions, detect_forbidden_patterns, detect_god_objects,
    detect_hallucinated_dependencies, detect_hollow_todos, detect_low_complexity,
    detect_missing_files, detect_missing_symbols, detect_missing_tests, detect_mock_data,
    detect_stub_functions, filter_suppressed, DetectionResult, GodObjectConfig,
    StubDetectionConfig,
};

/// Progress callback type for reporting file processing progress.
pub type ProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

/// Executes all detection checks against a set of files.
pub struct Runner {
    base_dir: PathBuf,
    skip_registry_check: bool,
    progress_callback: Option<ProgressCallback>,
}

impl Runner {
    /// Create a new detection runner.
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            skip_registry_check: false,
            progress_callback: None,
        }
    }

    /// Set whether to skip registry checks for dependency verification.
    pub fn skip_registry_check(mut self, skip: bool) -> Self {
        self.skip_registry_check = skip;
        self
    }

    /// Set a progress callback that will be called as files are processed.
    /// The callback receives (current_count, total_count).
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
        self
    }

    /// Run all detection checks defined in the contract.
    pub fn run(&self, files: &[PathBuf], contract: &Contract) -> anyhow::Result<DetectionResult> {
        let mut result = DetectionResult::new();
        let total_files = files.len();
        let processed = Arc::new(AtomicUsize::new(0));

        // Collect suppressions from all files (parallelized)
        let suppression_map = collect_suppressions(files)?;
        let all_suppressions: Vec<_> = suppression_map.values().flatten().cloned().collect();

        // Check required files (not file-parallel, quick)
        let file_result = detect_missing_files(&self.base_dir, &contract.required_files)?;
        result.merge(file_result);

        // Build god object config if enabled
        let god_config = contract.god_objects.as_ref().and_then(|god_cfg| {
            if god_cfg.is_enabled() {
                let defaults = GodObjectConfig::default();
                Some(GodObjectConfig {
                    max_file_lines: god_cfg.max_file_lines.unwrap_or(defaults.max_file_lines),
                    max_function_lines: god_cfg
                        .max_function_lines
                        .unwrap_or(defaults.max_function_lines),
                    max_function_complexity: god_cfg
                        .max_function_complexity
                        .unwrap_or(defaults.max_function_complexity),
                    max_functions_per_file: god_cfg
                        .max_functions_per_file
                        .unwrap_or(defaults.max_functions_per_file),
                    max_class_methods: god_cfg
                        .max_class_methods
                        .unwrap_or(defaults.max_class_methods),
                })
            } else {
                None
            }
        });

        // Run per-file detectors in parallel
        let detect_todos = contract.detect_hollow_todos();
        let patterns = &contract.forbidden_patterns;
        let mock_config = contract.mock_signatures.as_ref();
        let progress_cb = self.progress_callback.clone();
        let processed_clone = processed.clone();

        let file_results: Vec<DetectionResult> = files
            .par_iter()
            .map(|file| {
                let mut file_result = DetectionResult::new();

                // Forbidden patterns
                if !patterns.is_empty() {
                    if let Ok(r) = detect_forbidden_patterns(&[file.clone()], patterns) {
                        file_result.merge(r);
                    }
                }

                // Mock data
                if let Ok(r) = detect_mock_data(&[file.clone()], mock_config) {
                    file_result.merge(r);
                }

                // Hollow TODOs
                if detect_todos {
                    if let Ok(r) = detect_hollow_todos(&[file.clone()]) {
                        file_result.merge(r);
                    }
                }

                // God objects
                if let Some(ref config) = god_config {
                    if let Ok(r) = detect_god_objects(&[file.clone()], config) {
                        file_result.merge(r);
                    }
                }

                // Update progress
                let current = processed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(ref cb) = progress_cb {
                    cb(current, total_files);
                }

                file_result
            })
            .collect();

        // Merge all file results
        for r in file_results {
            result.merge(r);
        }

        // Non-parallelizable checks (require cross-file context)
        // Create analysis context for AST-backed detection
        let analysis_ctx = AnalysisContext::new(&self.base_dir);

        // Check required symbols (uses AST-backed analysis)
        let symbol_result =
            detect_missing_symbols(&analysis_ctx, files, &contract.required_symbols)?;
        result.merge(symbol_result);

        // Check complexity requirements (uses AST-backed analysis)
        let complexity_result = detect_low_complexity(&analysis_ctx, files, &contract.complexity)?;
        result.merge(complexity_result);

        // Check for stub functions using AST analysis
        // This uses the new tree-sitter based analyzer for precise detection
        let stub_config = StubDetectionConfig::default_enabled();
        let stub_result = detect_stub_functions(files, Some(&stub_config))?;
        result.merge(stub_result);

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

        // Deduplicate violations before applying suppressions
        result.deduplicate();

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
    use crate::contract::{ForbiddenPattern, HollowTodosConfig, RequiredFile};
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
            // Disable hollow TODO detection for this test
            hollow_todos: Some(HollowTodosConfig { enabled: false }),
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
            // Disable hollow TODO detection for this test
            hollow_todos: Some(HollowTodosConfig { enabled: false }),
            ..Default::default()
        };

        let runner = Runner::new(temp.path());
        let result = runner.run(&[main_go], &contract).unwrap();

        // The work marker should be suppressed
        assert_eq!(result.violations.len(), 0);
        assert_eq!(result.suppressed.len(), 1);
    }
}

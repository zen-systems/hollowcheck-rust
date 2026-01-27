//! Command-line interface for hollowcheck.

use clap::{Parser, Subcommand};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

use crate::contract::{self, Contract};
use crate::detect::Runner;
use crate::parser;
use crate::report;
use crate::score;

/// Exit codes.
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_ERROR: i32 = 2;

/// Default contract file names to search for.
const DEFAULT_CONTRACT_NAMES: &[&str] = &["hollowcheck.yaml", "hollow.yaml", ".hollowcheck.yaml"];

/// AI output quality gate system - detect hollow code implementations.
///
/// Hollowcheck validates AI-generated code against quality contracts.
/// It detects "hollow" code - implementations that look complete but lack
/// real functionality: stub implementations, placeholder data, unfinished
/// work markers, and functions with suspiciously low complexity.
#[derive(Parser)]
#[command(name = "hollowcheck")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check code quality against a contract
    #[command(visible_alias = "check")]
    Lint(LintArgs),
    /// Create a new hollowcheck contract from a template
    Init(InitArgs),
}

/// Arguments for the lint command.
#[derive(Parser)]
pub struct LintArgs {
    /// Path to check (file or directory)
    pub path: PathBuf,

    /// Path to contract YAML file (default: auto-discover)
    #[arg(short, long)]
    pub contract: Option<PathBuf>,

    /// Output format: pretty, json, or sarif
    #[arg(short, long, default_value = "pretty")]
    pub format: String,

    /// Maximum acceptable hollowness score (exit non-zero if exceeded)
    #[arg(short, long)]
    pub threshold: Option<i32>,

    /// Analysis mode: code (default) or prose
    #[arg(short, long)]
    pub mode: Option<String>,

    /// Show suppressed violations in output
    #[arg(long)]
    pub show_suppressed: bool,

    /// Skip dependency verification (registry lookups)
    #[arg(long)]
    pub skip_registry_check: bool,

    /// Use strict thresholds for AI-generated code (more aggressive detection)
    #[arg(long)]
    pub strict: bool,

    /// Use relaxed thresholds for large, mature codebases
    #[arg(long)]
    pub relaxed: bool,

    /// Additional glob patterns to exclude from analysis (can be specified multiple times)
    #[arg(long = "exclude", value_name = "PATTERN")]
    pub exclude_patterns: Vec<String>,

    /// Include files matching these patterns even if they would normally be excluded
    #[arg(long = "include", value_name = "PATTERN")]
    pub include_patterns: Vec<String>,
}

/// Arguments for the init command.
#[derive(Parser)]
pub struct InitArgs {
    /// Output file path
    #[arg(short, long, default_value = "hollowcheck.yaml")]
    pub output: PathBuf,

    /// Template to use
    #[arg(short, long, default_value = "minimal")]
    pub template: String,

    /// List available templates
    #[arg(short, long)]
    pub list: bool,
}

/// Available contract templates.
struct Template {
    name: &'static str,
    description: &'static str,
    content: &'static str,
}

/// All available templates.
static TEMPLATES: &[Template] = &[
    Template {
        name: "minimal",
        description: "Bare minimum quality gate - no stubs, no obvious mocks",
        content: include_str!("templates/minimal.yaml"),
    },
    Template {
        name: "crud-endpoint",
        description: "REST API endpoint with CRUD database operations",
        content: include_str!("templates/crud-endpoint.yaml"),
    },
    Template {
        name: "cli-tool",
        description: "Command-line tool with argument parsing and subcommands",
        content: include_str!("templates/cli-tool.yaml"),
    },
    Template {
        name: "client-sdk",
        description: "API client SDK with authentication, retry, and error handling",
        content: include_str!("templates/client-sdk.yaml"),
    },
    Template {
        name: "worker",
        description: "Background job processor with graceful shutdown",
        content: include_str!("templates/worker.yaml"),
    },
];

/// Discover a contract file in the current directory.
/// Returns None if no contract file is found.
fn discover_contract() -> Option<PathBuf> {
    for name in DEFAULT_CONTRACT_NAMES {
        let path = PathBuf::from(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Default directory patterns to exclude from scanning.
const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    // Build artifacts
    "target",
    "node_modules",
    "vendor",
    "dist",
    "build",
    "_build",
    // Test directories
    "test",
    "tests",
    "__tests__",
    "__test__",
    "testdata",
    "test_data",
    "test-data",
    // Example directories
    "example",
    "examples",
    // Benchmark directories
    "bench",
    "benches",
    "benchmark",
    "benchmarks",
    // Documentation
    "doc",
    "docs",
    "documentation",
];

/// Check if a filename indicates a test file.
fn is_test_file(filename: &str) -> bool {
    // Common test file patterns across languages
    filename.ends_with("_test.go")
        || filename.ends_with("_test.rs")
        || filename.ends_with("_test.py")
        || filename.ends_with("_test.js")
        || filename.ends_with("_test.ts")
        || filename.ends_with("_test.tsx")
        || filename.ends_with("_test.jsx")
        || filename.ends_with(".test.js")
        || filename.ends_with(".test.ts")
        || filename.ends_with(".test.tsx")
        || filename.ends_with(".test.jsx")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".spec.ts")
        || filename.ends_with(".spec.tsx")
        || filename.ends_with(".spec.jsx")
        || filename.starts_with("test_")
        || filename == "conftest.py"
}

/// Collect files to scan with additional exclude/include patterns.
fn collect_files_with_patterns(
    root: &Path,
    contract: &Contract,
    extra_excludes: &[String],
    include_patterns: &[String],
) -> anyhow::Result<Vec<PathBuf>> {
    let supported_extensions = [
        "go", "rs", "py", "js", "ts", "jsx", "tsx", "java", "kt", "c", "cpp", "h", "hpp",
    ];

    let include_test_files = contract.should_include_test_files();
    let mut files = Vec::new();

    // Build extra exclude matchers
    let extra_matchers: Vec<_> = extra_excludes
        .iter()
        .filter_map(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()))
        .collect();

    // Build include matchers
    let include_matchers: Vec<_> = include_patterns
        .iter()
        .filter_map(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()))
        .collect();

    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden directories
            if e.file_type().is_dir() && name.starts_with('.') {
                return false;
            }
            // Skip default excluded directories
            if e.file_type().is_dir() && DEFAULT_EXCLUDED_DIRS.contains(&name.as_ref()) {
                return false;
            }
            true
        })
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            if supported_extensions.contains(&ext) {
                let path_str = path.to_string_lossy();
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Check if file matches include patterns (always include these)
                let force_include = include_matchers.iter().any(|m| m.is_match(&*path_str));

                if !force_include {
                    // Skip test files unless explicitly included
                    if !include_test_files && is_test_file(filename) {
                        continue;
                    }

                    // Skip files matching excluded_paths patterns from contract
                    if contract.is_path_excluded(path) {
                        continue;
                    }

                    // Skip files matching extra exclude patterns from CLI
                    if extra_matchers.iter().any(|m| m.is_match(&*path_str)) {
                        continue;
                    }
                }

                files.push(path.to_path_buf());
            }
        }
    }

    Ok(files)
}

/// Run the lint command.
pub fn run_lint(args: &LintArgs) -> anyhow::Result<i32> {
    let start_time = Instant::now();
    let is_interactive = args.format == "pretty";

    // Show progress only in interactive mode
    let progress_msg = |msg: &str| {
        if is_interactive {
            eprintln!("{}", msg.dimmed());
        }
    };

    // Validate format
    if args.format != "pretty" && args.format != "json" && args.format != "sarif" {
        eprintln!(
            "Error: invalid format {:?}, must be 'pretty', 'json', or 'sarif'",
            args.format
        );
        return Ok(EXIT_ERROR);
    }

    // Validate mode
    let mode = args.mode.as_deref().unwrap_or("code");
    if mode != "code" && mode != "prose" {
        eprintln!("Error: invalid mode {:?}, must be 'code' or 'prose'", mode);
        return Ok(EXIT_ERROR);
    }

    // Phase 1: Initialization
    progress_msg("Initializing parsers...");
    let init_start = Instant::now();
    parser::init();
    if is_interactive && init_start.elapsed().as_secs_f32() > 0.5 {
        eprintln!("  {} Loaded parsers ({:.1}s)", "✓".green(), init_start.elapsed().as_secs_f32());
    }

    // Validate strict/relaxed flags are not both set
    if args.strict && args.relaxed {
        eprintln!("Error: cannot use both --strict and --relaxed flags");
        return Ok(EXIT_ERROR);
    }

    // Discover contract if not specified, or use default if none found
    let (contract_path, mut contract) = match &args.contract {
        Some(p) => {
            // Explicit contract specified - must exist
            match Contract::parse_file(p) {
                Ok(c) => (p.to_string_lossy().to_string(), c),
                Err(e) => {
                    eprintln!("Error parsing contract: {}", e);
                    return Ok(EXIT_ERROR);
                }
            }
        }
        None => {
            // No explicit contract - try to discover, or use default
            match discover_contract() {
                Some(p) => {
                    match Contract::parse_file(&p) {
                        Ok(c) => (p.to_string_lossy().to_string(), c),
                        Err(e) => {
                            eprintln!("Error parsing contract: {}", e);
                            return Ok(EXIT_ERROR);
                        }
                    }
                }
                None => {
                    // No contract found - use default
                    if is_interactive {
                        eprintln!("{} No contract file found, using default settings", "ℹ".blue());
                    }
                    ("<default>".to_string(), Contract::default_contract())
                }
            }
        }
    };

    // Apply strict/relaxed thresholds if specified
    if args.strict || args.relaxed {
        use crate::detect::GodObjectConfig;
        let thresholds = if args.strict {
            GodObjectConfig::strict()
        } else {
            GodObjectConfig::relaxed()
        };

        // Update contract's god_objects config with the selected thresholds
        let god_cfg = contract.god_objects.get_or_insert(Default::default());
        if god_cfg.max_file_lines.is_none() {
            god_cfg.max_file_lines = Some(thresholds.max_file_lines);
        }
        if god_cfg.max_function_lines.is_none() {
            god_cfg.max_function_lines = Some(thresholds.max_function_lines);
        }
        if god_cfg.max_function_complexity.is_none() {
            god_cfg.max_function_complexity = Some(thresholds.max_function_complexity);
        }
        if god_cfg.max_functions_per_file.is_none() {
            god_cfg.max_functions_per_file = Some(thresholds.max_functions_per_file);
        }
        if god_cfg.max_class_methods.is_none() {
            god_cfg.max_class_methods = Some(thresholds.max_class_methods);
        }
    }

    // Validate contract
    if let Err(e) = contract::validate(&contract) {
        eprintln!("Error: invalid contract: {}", e);
        return Ok(EXIT_ERROR);
    }

    // Resolve path
    let abs_path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot access path {:?}: {}", args.path, e);
            return Ok(EXIT_ERROR);
        }
    };

    // Check path exists
    let metadata = match std::fs::metadata(&abs_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: {}", e);
            return Ok(EXIT_ERROR);
        }
    };

    // Phase 2: File collection
    progress_msg("Scanning files...");
    let collect_start = Instant::now();
    let files = if metadata.is_dir() {
        collect_files_with_patterns(&abs_path, &contract, &args.exclude_patterns, &args.include_patterns)?
    } else {
        vec![abs_path.clone()]
    };

    if files.is_empty() {
        eprintln!("Warning: no files to scan");
        return Ok(EXIT_SUCCESS);
    }

    if is_interactive {
        eprintln!("  {} Found {} files ({:.1}s)", "✓".green(), files.len(), collect_start.elapsed().as_secs_f32());
    }

    // Phase 3: Analysis with progress bar for large file counts
    let analysis_start = Instant::now();
    let result = if is_interactive && files.len() > 10 {
        // Show progress bar for larger codebases
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        // Clone pb for the closure
        let pb_clone = pb.clone();

        // Run detection with progress callback
        let runner = Runner::new(&abs_path)
            .skip_registry_check(args.skip_registry_check)
            .with_progress(move |current, _total| {
                pb_clone.set_position(current as u64);
            });
        let result = runner.run(&files, &contract)?;

        pb.finish_and_clear();
        eprintln!("  {} Analysis complete ({:.1}s)", "✓".green(), analysis_start.elapsed().as_secs_f32());
        result
    } else {
        // No progress bar for small file counts
        let runner = Runner::new(&abs_path).skip_registry_check(args.skip_registry_check);
        runner.run(&files, &contract)?
    };

    if is_interactive && start_time.elapsed().as_secs_f32() > 1.0 {
        eprintln!("  {} Total time: {:.1}s", "✓".green(), start_time.elapsed().as_secs_f32());
        eprintln!();
    }

    // Calculate score
    let hollowness = if let Some(threshold) = args.threshold {
        score::calculate_with_threshold(&result, threshold)
    } else {
        score::calculate(&result, &contract)
    };

    // Output results
    let path_str = args.path.to_string_lossy().to_string();

    match args.format.as_str() {
        "json" => {
            report::write_json(&path_str, &contract_path, &result, &hollowness)?;
        }
        "sarif" => {
            report::write_sarif(&abs_path, &result)?;
        }
        _ => {
            report::write_pretty(
                &path_str,
                &contract_path,
                &result,
                &hollowness,
                args.show_suppressed,
            );
        }
    }

    // Return appropriate exit code
    if hollowness.passed {
        Ok(EXIT_SUCCESS)
    } else {
        Ok(EXIT_FAILED)
    }
}

/// Run the init command.
pub fn run_init(args: &InitArgs) -> anyhow::Result<i32> {
    // List mode
    if args.list {
        return list_templates();
    }

    // Find template
    let template = match TEMPLATES.iter().find(|t| t.name == args.template) {
        Some(t) => t,
        None => {
            eprintln!("Error: unknown template {:?}", args.template);
            eprintln!("Run 'hollowcheck init --list' to see available templates");
            return Ok(EXIT_ERROR);
        }
    };

    // Check if output already exists
    if args.output.exists() {
        eprintln!("Error: file already exists: {}", args.output.display());
        eprintln!("Remove it or use --output to specify a different path");
        return Ok(EXIT_ERROR);
    }

    // Create output directory if needed
    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() && parent != Path::new(".") {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("Error: failed to create directory: {}", e);
                return Ok(EXIT_ERROR);
            }
        }
    }

    // Write contract file
    if let Err(e) = std::fs::write(&args.output, template.content) {
        eprintln!("Error: failed to write contract: {}", e);
        return Ok(EXIT_ERROR);
    }

    // Success message
    println!(
        "Created {} from template '{}'",
        args.output.display(),
        template.name
    );
    println!();
    println!("Next steps:");
    println!(
        "  1. Edit {} to customize for your project",
        args.output.display()
    );
    println!(
        "  2. Run: hollowcheck lint . --contract {}",
        args.output.display()
    );

    Ok(EXIT_SUCCESS)
}

/// List available templates.
fn list_templates() -> anyhow::Result<i32> {
    println!("Available templates:");
    println!();

    for template in TEMPLATES {
        let name = if template.name == "minimal" {
            format!("{} (default)", template.name)
        } else {
            template.name.to_string()
        };
        println!("  {:<20} {}", name, template.description);
    }

    println!();
    println!("Usage:");
    println!("  hollowcheck init --template <name>");

    Ok(EXIT_SUCCESS)
}

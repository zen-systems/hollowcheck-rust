//! Command-line interface for hollowcheck.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
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
fn discover_contract() -> anyhow::Result<PathBuf> {
    for name in DEFAULT_CONTRACT_NAMES {
        let path = PathBuf::from(name);
        if path.exists() {
            return Ok(path);
        }
    }
    anyhow::bail!(
        "no contract file found (looked for {})",
        DEFAULT_CONTRACT_NAMES.join(", ")
    )
}

/// Collect files to scan based on mode.
fn collect_files(root: &Path, include_test_files: bool) -> anyhow::Result<Vec<PathBuf>> {
    let supported_extensions = [
        "go", "rs", "py", "js", "ts", "jsx", "tsx", "java", "kt", "c", "cpp", "h", "hpp",
    ];

    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip hidden directories
            if e.file_type().is_dir() && name.starts_with('.') {
                return false;
            }
            // Skip vendor, node_modules, and test directories
            if e.file_type().is_dir()
                && (name == "vendor"
                    || name == "node_modules"
                    || name == "testdata"
                    || name == "test_data"
                    || name == "tests"
                    || name == "test"
                    || name == "__tests__"
                    || name == "__test__")
            {
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
                // Skip test files unless explicitly included
                if !include_test_files {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.ends_with("_test.go") {
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
    // Initialize tree-sitter parsers (no-op if feature disabled)
    parser::init();

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

    // Discover contract if not specified
    let contract_path = match &args.contract {
        Some(p) => p.clone(),
        None => match discover_contract() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Run 'hollowcheck init' to create a contract file");
                return Ok(EXIT_ERROR);
            }
        },
    };

    // Parse contract
    let contract = match Contract::parse_file(&contract_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error parsing contract: {}", e);
            return Ok(EXIT_ERROR);
        }
    };

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

    // Collect files to scan
    let files = if metadata.is_dir() {
        collect_files(&abs_path, contract.should_include_test_files())?
    } else {
        vec![abs_path.clone()]
    };

    if files.is_empty() {
        eprintln!("Warning: no files to scan");
        return Ok(EXIT_SUCCESS);
    }

    // Run detection
    let runner = Runner::new(&abs_path).skip_registry_check(args.skip_registry_check);
    let result = runner.run(&files, &contract)?;

    // Calculate score
    let hollowness = if let Some(threshold) = args.threshold {
        score::calculate_with_threshold(&result, threshold)
    } else {
        score::calculate(&result, &contract)
    };

    // Output results
    let contract_path_str = contract_path.to_string_lossy().to_string();
    let path_str = args.path.to_string_lossy().to_string();

    match args.format.as_str() {
        "json" => {
            report::write_json(&path_str, &contract_path_str, &result, &hollowness)?;
        }
        "sarif" => {
            report::write_sarif(&abs_path, &result)?;
        }
        _ => {
            report::write_pretty(
                &path_str,
                &contract_path_str,
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
    println!("Created {} from template '{}'", args.output.display(), template.name);
    println!();
    println!("Next steps:");
    println!("  1. Edit {} to customize for your project", args.output.display());
    println!("  2. Run: hollowcheck lint . --contract {}", args.output.display());

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

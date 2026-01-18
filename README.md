# Hollowcheck

AI output quality gate system - detect hollow code implementations.

Hollowcheck validates AI-generated code against quality contracts. It detects "hollow" code - implementations that look complete but lack real functionality: stub implementations, placeholder data, TODO markers, functions with suspiciously low complexity, and hallucinated dependencies that don't exist in package registries.

> **Note:** This is a Rust rewrite of the [original Go implementation](https://github.com/zen-systems/hollowcheck). The Rust version provides identical functionality with improved performance and cross-platform binary distribution.

## Installation

### Download Binary

Download the latest release for your platform from the [Releases page](https://github.com/zen-systems/hollowcheck-rust/releases).

| Platform | Binary |
|----------|--------|
| Linux (x86_64) | `hollowcheck-linux-amd64` |
| Linux (ARM64) | `hollowcheck-linux-arm64` |
| macOS (Intel) | `hollowcheck-darwin-amd64` |
| macOS (Apple Silicon) | `hollowcheck-darwin-arm64` |
| Windows (x86_64) | `hollowcheck-windows-amd64.exe` |

```bash
# Linux/macOS
chmod +x hollowcheck-*
sudo mv hollowcheck-* /usr/local/bin/hollowcheck

# Verify installation
hollowcheck --version
```

### Build from Source

Requires [Rust](https://rustup.rs/) 1.70+.

```bash
# Clone the repository
git clone https://github.com/zen-systems/hollowcheck-rust.git
cd hollowcheck-rust

# Build and install
cargo install --path .

# Or build release binary
cargo build --release
# Binary will be at target/release/hollowcheck
```

### Cargo Install

```bash
cargo install hollowcheck
```

## Quick Start

1. **Initialize a contract:**

```bash
hollowcheck init
# Creates hollowcheck.yaml with minimal template

# Or use a specific template
hollowcheck init --template crud-endpoint
hollowcheck init --list  # See all available templates
```

2. **Run analysis:**

```bash
hollowcheck lint .
# Or specify a contract
hollowcheck lint . --contract my-contract.yaml
```

3. **View results:**

```
  hollowcheck v0.1.0

  Scanning: .
  Contract: hollowcheck.yaml

  ✗ FAIL  Hollowness: 45%  Grade: C

  Violations (5):

    ERROR  forbidden_pattern  src/handler.go:42
            Found 'TODO: implement validation'

    ERROR  low_complexity     src/handler.go
            Function 'ProcessRequest' has complexity 1, expected >= 3

    WARN   mock_data          src/config.go:15
            Found mock domain 'example.com'

  Breakdown:
    forbidden_pattern      20 pts (2 violations)
    low_complexity         10 pts (1 violation)
    mock_data              15 pts (5 violations)

  Threshold: 25  Score: 45  FAILED
```

## Contract Schema

Contracts define quality expectations for your code:

```yaml
version: "1.0"
name: "my-project"
description: "Quality contract for my project"

# Files that must exist
required_files:
  - path: "main.go"
    required: true
  - path: "handler.go"
    required: true

# Symbols that must be defined
required_symbols:
  - name: "ProcessRequest"
    kind: function
    file: "handler.go"
  - name: "Config"
    kind: type
    file: "config.go"

# Patterns that should not appear
forbidden_patterns:
  - pattern: "TODO"
    description: "Remove TODO comments before shipping"
  - pattern: "FIXME"
    description: "Fix all FIXME items"
  - pattern: 'panic\("not implemented"\)'
    description: "Replace stub implementations"

# Mock data that shouldn't be in production code
mock_signatures:
  skip_test_files: true
  patterns:
    - pattern: 'example\.com'
      description: "Mock domain"
    - pattern: '"12345"|"00000"'
      description: "Fake IDs"
    - pattern: "lorem ipsum"
      description: "Placeholder text"

# Minimum complexity requirements (catch stub functions)
complexity:
  - symbol: "ProcessRequest"
    file: "handler.go"
    min_complexity: 3
  - symbol: "ValidateInput"
    file: "handler.go"
    min_complexity: 2

# Required test functions
required_tests:
  - name: "TestProcessRequest"
    file: "handler_test.go"

# Verify dependencies exist in package registries
dependency_verification:
  enabled: true
  registries:
    pypi:
      enabled: true
    npm:
      enabled: true
    crates:
      enabled: true
    go:
      enabled: true
  allowlist:
    - "internal-*"        # Glob patterns for internal packages
    - "company-*"
  cache_ttl_hours: 24     # Cache registry results
  fail_on_timeout: false  # Don't fail on network errors
```

## Available Templates

| Template | Description |
|----------|-------------|
| `minimal` | Bare minimum quality gate (default) |
| `crud-endpoint` | REST API with CRUD database operations |
| `cli-tool` | Command-line tool with argument parsing |
| `client-sdk` | API client SDK with auth/retry/error handling |
| `worker` | Background job processor with graceful shutdown |

## Hallucinated Dependency Detection

AI models sometimes generate imports for packages that don't exist. Hollowcheck can verify that imported dependencies actually exist in their respective package registries.

### Supported Registries

| Language | Registry | Extensions |
|----------|----------|------------|
| Python | PyPI | `.py` |
| JavaScript/TypeScript | npm | `.js`, `.jsx`, `.ts`, `.tsx` |
| Go | Go Proxy | `.go` |
| Rust | crates.io | `.rs` |

### How It Works

1. **Import Extraction**: Parses source files to extract import statements
2. **Standard Library Filtering**: Ignores known standard library modules
3. **Registry Lookup**: Checks each dependency against the appropriate registry
4. **Caching**: Results are cached locally for 24 hours (configurable)
5. **Allowlist Matching**: Internal/private packages can be allowlisted with glob patterns

### Example Output

```
  ERROR  hallucinated_dependency  src/utils.py:5
          package "nonexistent-ai-package" not found in pypi

  ERROR  hallucinated_dependency  src/client.js:3
          package "fake-http-utils" not found in npm
```

### Allowlist Patterns

Use glob patterns to allowlist internal packages that won't be found in public registries:

```yaml
dependency_verification:
  allowlist:
    - "internal-*"           # All packages starting with "internal-"
    - "@company/*"           # All scoped npm packages under @company
    - "github.com/company/*" # All Go modules under your org
```

## Output Formats

### Pretty (default)
Human-readable colored terminal output.

```bash
hollowcheck lint . --format pretty
```

### JSON
Structured output for programmatic consumption.

```bash
hollowcheck lint . --format json
```

### SARIF
Static Analysis Results Interchange Format for IDE/CI integration.

```bash
hollowcheck lint . --format sarif
```

## CI Integration

### GitHub Actions

```yaml
- name: Run Hollowcheck
  run: |
    curl -L https://github.com/zen-systems/hollowcheck-rust/releases/latest/download/hollowcheck-linux-amd64 -o hollowcheck
    chmod +x hollowcheck
    ./hollowcheck lint . --format sarif > results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: results.sarif
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Analysis passed (score ≤ threshold) |
| 1 | Analysis failed (score > threshold) |
| 2 | Error (invalid contract, missing files, etc.) |

### Threshold Override

Set a custom threshold for CI:

```bash
hollowcheck lint . --threshold 50
```

### Skip Registry Checks

Skip dependency verification for offline use or faster runs:

```bash
hollowcheck lint . --skip-registry-check
```

## Inline Suppressions

Suppress violations with inline comments:

```go
// hollowcheck:ignore forbidden_pattern - Intentional TODO for tracking
// TODO: Add logging

// hollowcheck:ignore-next-line mock_data - Test fixture
var testURL = "example.com"

// hollowcheck:ignore-file low_complexity - Generated code
```

## Scoring

| Violation | Points |
|-----------|--------|
| Missing file | 20 |
| Missing symbol | 15 |
| Hallucinated dependency | 15 |
| Forbidden pattern | 10 |
| Low complexity | 10 |
| Missing test | 5 |
| Mock data | 3 |

| Grade | Score Range |
|-------|-------------|
| A | 0-10 |
| B | 11-25 |
| C | 26-50 |
| D | 51-75 |
| F | 76-100 |

Default threshold: **25** (Grade B or better passes)

## Development

### Building

```bash
cargo build
cargo build --release
```

### Testing

```bash
cargo test
cargo test --release
```

### Running locally

```bash
cargo run -- lint testdata --contract testdata/test-contract.yaml
```

## License

Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Credits

- Original Go implementation: [zen-systems/hollowcheck](https://github.com/zen-systems/hollowcheck)
- Tree-sitter for AST parsing
- Inspired by the need to validate AI-generated code quality

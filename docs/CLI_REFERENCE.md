# Hollowcheck CLI Reference

Hollowcheck is an AI output quality gate that validates code against quality contracts, detecting hollow implementations, stub functions, mock data, and hallucinated dependencies.

## Installation

### From Binary

```bash
# Download latest release
curl -LO https://github.com/yourorg/hollowcheck/releases/latest/download/hollowcheck-$(uname -s)-$(uname -m)
chmod +x hollowcheck-*
sudo mv hollowcheck-* /usr/local/bin/hollowcheck
```

### From Source

```bash
cargo install --path .
```

### Via Cargo

```bash
cargo install hollowcheck
```

## Commands

### `hollowcheck lint`

Check code quality against a contract.

```bash
hollowcheck lint [OPTIONS] <PATH>
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<PATH>` | Directory or file to check |

**Options:**

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-c, --contract` | string | `.hollowcheck.yaml` | Path to contract YAML file |
| `-f, --format` | string | `pretty` | Output format: `pretty`, `json`, `sarif` |
| `-t, --threshold` | int | (from contract) | Override score threshold |
| `--strict` | bool | `false` | Use strict thresholds (lower tolerance) |
| `--relaxed` | bool | `false` | Use relaxed thresholds (higher tolerance) |
| `--skip-registry-check` | bool | `false` | Skip dependency verification against registries |
| `--exclude` | string[] | | Glob patterns to exclude |
| `--include` | string[] | | Glob patterns to include (overrides excludes) |
| `--show-suppressed` | bool | `false` | Show suppressed violations in output |

**Examples:**

```bash
# Basic usage with default contract
hollowcheck lint .

# Specify contract file
hollowcheck lint --contract my-contract.yaml ./src

# JSON output for CI
hollowcheck lint --format json . > report.json

# SARIF output for IDE integration
hollowcheck lint --format sarif . > report.sarif

# Strict mode for production
hollowcheck lint --strict .

# Skip slow registry checks
hollowcheck lint --skip-registry-check .

# Exclude generated files
hollowcheck lint --exclude "**/generated/**" --exclude "**/vendor/**" .

# Override threshold
hollowcheck lint --threshold 50 .
```

---

### `hollowcheck init`

Create a new contract file from a template.

```bash
hollowcheck init [OPTIONS]
```

**Options:**

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-t, --template` | string | `minimal` | Template name |
| `-o, --output` | string | `.hollowcheck.yaml` | Output file path |
| `--list` | bool | `false` | List available templates |

**Available Templates:**

| Template | Description |
|----------|-------------|
| `minimal` | Bare minimum contract (default) |
| `crud-endpoint` | REST API with CRUD operations |
| `cli-tool` | Command-line tool |
| `client-sdk` | API client SDK |
| `worker` | Background job processor |

**Examples:**

```bash
# Create minimal contract
hollowcheck init

# Create CRUD endpoint contract
hollowcheck init --template crud-endpoint --output api-contract.yaml

# List templates
hollowcheck init --list
```

---

## Output Formats

### Pretty (Default)

Colored terminal output with violation details:

```
hollowcheck v0.5.0
═══════════════════════════════════════════════════════════════════

Scanning 42 files...

✗ CRITICAL: required symbol "ProcessRequest" not found
  → src/handler.go:0

✗ HIGH: forbidden pattern "TODO" found
  → src/service.go:45

✗ MEDIUM: function "validateInput" appears to be a stub
  → src/validator.go:23

───────────────────────────────────────────────────────────────────
Score: 35/100 (Grade: C)
Threshold: 25
Status: FAIL

3 violations found
  Critical: 1
  High: 1
  Medium: 1
```

### JSON

Structured JSON for programmatic use:

```json
{
  "version": "0.5.0",
  "score": 35,
  "grade": "C",
  "threshold": 25,
  "passed": false,
  "violations": [
    {
      "rule": "missing_symbol",
      "severity": "critical",
      "message": "required symbol \"ProcessRequest\" not found",
      "file": "src/handler.go",
      "line": 0
    }
  ],
  "summary": {
    "files_scanned": 42,
    "violations_total": 3,
    "by_severity": {
      "critical": 1,
      "high": 1,
      "medium": 1
    }
  }
}
```

### SARIF

Static Analysis Results Interchange Format for CI/IDE integration:

```json
{
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "hollowcheck",
          "version": "0.5.0"
        }
      },
      "results": [...]
    }
  ]
}
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Pass - score ≤ threshold |
| `1` | Fail - score > threshold |
| `2` | Error - invalid input, contract, or configuration |

---

## Inline Suppressions

Suppress specific violations with inline comments:

```go
// hollowcheck:ignore-next-line forbidden_pattern - Expected TODO for tracking
// TODO: implement error handling

// hollowcheck:ignore-next-line stub_function - Intentionally minimal
func placeholder() {}
```

**Suppression Format:**

```
hollowcheck:ignore-next-line <rule> - <reason>
```

**Rules that can be suppressed:**

| Rule | Description |
|------|-------------|
| `forbidden_pattern` | Forbidden text pattern |
| `stub_function` | Stub function detection |
| `low_complexity` | Low complexity function |
| `mock_data` | Mock/placeholder data |
| `hollow_todo` | Hollow TODO comment |
| `god_object` | God object detection |

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `HOLLOWCHECK_CONTRACT` | Default contract path |
| `HOLLOWCHECK_THRESHOLD` | Default score threshold |
| `NO_COLOR` | Disable colored output |

---

## CI Integration

### GitHub Actions

```yaml
name: Code Quality
on: [push, pull_request]

jobs:
  hollowcheck:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Hollowcheck
        run: |
          curl -LO https://github.com/yourorg/hollowcheck/releases/latest/download/hollowcheck-Linux-x86_64
          chmod +x hollowcheck-Linux-x86_64
          sudo mv hollowcheck-Linux-x86_64 /usr/local/bin/hollowcheck

      - name: Run Hollowcheck
        run: hollowcheck lint --format sarif . > results.sarif

      - name: Upload SARIF
        uses: github/codeql-action/upload-sarif@v2
        with:
          sarif_file: results.sarif
```

### GitLab CI

```yaml
hollowcheck:
  stage: test
  script:
    - hollowcheck lint --format json . > hollowcheck-report.json
  artifacts:
    reports:
      codequality: hollowcheck-report.json
```

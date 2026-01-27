# Usage Guide

This guide covers common usage patterns and workflows for Hollowcheck.

## Quick Start

### 1. Create a Contract

```bash
# Create minimal contract
hollowcheck init

# Or use a template
hollowcheck init --template crud-endpoint --output .hollowcheck.yaml
```

### 2. Run the Check

```bash
hollowcheck lint .
```

### 3. Review Results

```
hollowcheck v0.5.0
═══════════════════════════════════════════════════════════════════

Scanning 42 files...

✓ All checks passed

───────────────────────────────────────────────────────────────────
Score: 0/100 (Grade: A)
Threshold: 25
Status: PASS
```

---

## Workflow: Validating AI-Generated Code

When using AI to generate code, run Hollowcheck before accepting the output:

### 1. Generate Code with AI

```bash
# Using flowgate or any AI tool
flowgate ask "Implement a rate limiter in Go" > rate_limiter.go
```

### 2. Define Quality Expectations

Create `.hollowcheck.yaml`:

```yaml
version: "1.0"
name: "rate-limiter"

required_files:
  - path: "rate_limiter.go"
    required: true
  - path: "rate_limiter_test.go"
    required: true

required_symbols:
  - name: "RateLimiter"
    kind: type
    file: "rate_limiter.go"
  - name: "Allow"
    kind: method
    file: "rate_limiter.go"
  - name: "NewRateLimiter"
    kind: function
    file: "rate_limiter.go"

forbidden_patterns:
  - pattern: "TODO"
  - pattern: "panic\\("

complexity:
  - symbol: "Allow"
    file: "rate_limiter.go"
    min_complexity: 3

required_tests:
  - name: "TestRateLimiter"
    file: "rate_limiter_test.go"

threshold: 10
```

### 3. Validate the Output

```bash
hollowcheck lint .
```

### 4. Iterate Until Passing

If violations are found, feed them back to the AI for fixing:

```bash
# Get violations as JSON
hollowcheck lint --format json . 2>&1 | jq '.violations'

# Feed back to AI
flowgate ask "Fix these issues in rate_limiter.go: [violations]"
```

---

## Workflow: CI/CD Integration

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
        run: hollowcheck lint .

      - name: Upload SARIF (optional)
        if: always()
        run: hollowcheck lint --format sarif . > results.sarif
        continue-on-error: true

      - name: Upload to GitHub Security
        uses: github/codeql-action/upload-sarif@v2
        if: always()
        with:
          sarif_file: results.sarif
```

### GitLab CI

```yaml
stages:
  - quality

hollowcheck:
  stage: quality
  image: rust:latest
  before_script:
    - cargo install hollowcheck
  script:
    - hollowcheck lint --format json . > hollowcheck-report.json
  artifacts:
    reports:
      codequality: hollowcheck-report.json
    when: always
```

### Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

hollowcheck lint . --threshold 25
if [ $? -ne 0 ]; then
    echo "Hollowcheck failed. Fix violations before committing."
    exit 1
fi
```

---

## Workflow: Incremental Adoption

Start with relaxed settings and gradually increase strictness:

### Phase 1: Discovery

```yaml
# .hollowcheck.yaml - Discovery phase
version: "1.0"
name: "discovery"

forbidden_patterns:
  - pattern: "TODO"

threshold: 100  # Very relaxed - just see what's there
```

```bash
hollowcheck lint . --format json > baseline.json
```

### Phase 2: Address Critical Issues

```yaml
# .hollowcheck.yaml - Address criticals
version: "1.0"
name: "critical-fixes"

required_files:
  - path: "main.go"
    required: true

forbidden_patterns:
  - pattern: "panic\\("
    description: "Use error handling"

dependency_verification:
  enabled: true

threshold: 50  # Still relaxed
```

### Phase 3: Full Enforcement

```yaml
# .hollowcheck.yaml - Production ready
version: "1.0"
name: "production"

# Full contract here...

god_objects:
  enabled: true
  max_function_lines: 50

hollow_todos:
  enabled: true

threshold: 25  # Strict
```

---

## Workflow: Multi-Language Projects

For projects with multiple languages, create a comprehensive contract:

```yaml
version: "1.0"
name: "multi-lang-project"

# Backend (Go)
required_files:
  - path: "cmd/server/main.go"
    required: true
  - path: "internal/handler/handler.go"
    required: true

required_symbols:
  - name: "main"
    kind: function
    file: "cmd/server/main.go"
  - name: "HandleRequest"
    kind: function
    file: "internal/handler/handler.go"

# Frontend (TypeScript)
required_files:
  - path: "web/src/App.tsx"
    required: true
  - path: "web/src/components/Dashboard.tsx"
    required: true

# Common patterns across all languages
forbidden_patterns:
  - pattern: "TODO"
  - pattern: "FIXME"
  - pattern: "console\\.log"
  - pattern: "fmt\\.Print"

# Verify dependencies for all languages
dependency_verification:
  enabled: true
  registries:
    go: { enabled: true }
    npm: { enabled: true }

threshold: 25
```

---

## Workflow: Suppressing False Positives

### Inline Suppression

```go
// hollowcheck:ignore-next-line forbidden_pattern - Intentional panic for unrecoverable errors
panic("database connection failed")

// hollowcheck:ignore-next-line stub_function - Interface implementation, logic in subclasses
func (b *BaseHandler) Handle() {}
```

### Contract-Level Exclusion

```yaml
excluded_paths:
  - "**/generated/**"
  - "**/vendor/**"
  - "**/mocks/**"
  - "**/testdata/**"
```

### Mock Data in Tests

```yaml
mock_signatures:
  skip_test_files: true  # Don't flag mock data in test files
```

---

## Workflow: Custom Templates

### Creating a Custom Template

1. Start with an existing template:
   ```bash
   hollowcheck init --template crud-endpoint --output my-template.yaml
   ```

2. Customize for your project:
   ```yaml
   version: "1.0"
   name: "{{project_name}}"
   description: "Custom template for our team"

   required_files:
     - path: "main.go"
       required: true
     - path: "README.md"
       required: true
     - path: "Makefile"
       required: true

   required_symbols:
     - name: "main"
       kind: function
       file: "main.go"
     - name: "Run"
       kind: function
       file: "cmd/run.go"

   forbidden_patterns:
     - pattern: "TODO"
     - pattern: "fmt\\.Print"
     - pattern: "os\\.Exit\\(1\\)"
       description: "Use proper error handling"

   # Team standards
   god_objects:
     enabled: true
     max_file_lines: 300
     max_function_lines: 30

   threshold: 20
   ```

3. Share with your team:
   ```bash
   cp my-template.yaml ~/.hollowcheck/templates/team-standard.yaml
   ```

---

## Output Formats

### Pretty (Terminal)

Best for local development:

```bash
hollowcheck lint .
```

### JSON (Programmatic)

Best for CI/CD and tooling:

```bash
hollowcheck lint --format json . | jq '.score'
```

### SARIF (IDE Integration)

Best for IDE and GitHub Security:

```bash
hollowcheck lint --format sarif . > results.sarif
```

---

## Performance Tips

### Skip Registry Checks

Registry checks can be slow. Skip them for quick local runs:

```bash
hollowcheck lint --skip-registry-check .
```

### Exclude Large Directories

```bash
hollowcheck lint --exclude "**/node_modules/**" --exclude "**/vendor/**" .
```

### Use Include for Targeted Checks

```bash
hollowcheck lint --include "src/**/*.go" .
```

---

## Troubleshooting

### "No contract file found"

Create a contract:
```bash
hollowcheck init
```

### "Unsupported file extension"

Check supported languages in `hollowcheck --help` or create a contract that excludes unsupported files:
```yaml
excluded_paths:
  - "**/*.xyz"
```

### "Registry timeout"

Use `--skip-registry-check` or configure:
```yaml
dependency_verification:
  fail_on_timeout: false
  cache_ttl_hours: 48
```

### High false positive rate

1. Use inline suppressions for legitimate cases
2. Adjust thresholds in contract
3. Exclude generated/vendor code

### Score seems wrong

Check which rules are contributing:
```bash
hollowcheck lint --format json . | jq '.violations | group_by(.rule) | map({rule: .[0].rule, count: length})'
```

# Contract Reference

Hollowcheck contracts define quality expectations for your codebase in YAML format.

## Basic Structure

```yaml
version: "1.0"
name: "my-project"
description: "Quality contract for my project"

# Quality checks
required_files: [...]
required_symbols: [...]
forbidden_patterns: [...]
complexity: [...]
required_tests: [...]

# Detection configuration
mock_signatures: {...}
god_objects: {...}
hollow_todos: {...}
dependency_verification: {...}

# Thresholds
threshold: 25
```

---

## Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | string | No | Contract schema version (default: "1.0") |
| `name` | string | No | Project identifier |
| `description` | string | No | Human-readable description |
| `mode` | string | No | Analysis mode: `code` (default) or `prose` |
| `include_test_files` | bool | No | Include test files in analysis (default: false) |
| `excluded_paths` | string[] | No | Glob patterns to exclude |
| `threshold` | int | No | Score threshold for pass/fail (default: 25) |

---

## Required Files

Verify that specific files exist in the codebase:

```yaml
required_files:
  - path: "main.go"
    required: true

  - path: "README.md"
    required: true

  - path: "docs/API.md"
    required: false    # Warning only, not critical
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | string | Required | Relative path to file |
| `required` | bool | `true` | Whether file is critical |

### Scoring

- Missing required file: **20 points** (Critical)
- Missing optional file: **5 points** (Warning)

---

## Required Symbols

Verify that specific functions, types, or constants exist:

```yaml
required_symbols:
  - name: "ProcessRequest"
    kind: function
    file: "handler.go"

  - name: "Handler"
    kind: type
    file: "handler.go"

  - name: "Version"
    kind: const
    file: "version.go"

  - name: "Handle"
    kind: method
    file: "handler.go"
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | Required | Symbol name |
| `kind` | string | Required | Symbol kind: `function`, `method`, `type`, `const` |
| `file` | string | Required | File where symbol should exist |

### Scoring

- Missing required symbol: **15 points** (Critical)

---

## Forbidden Patterns

Detect unwanted text patterns using regex:

```yaml
forbidden_patterns:
  - pattern: "TODO"
    description: "Remove TODO comments before shipping"

  - pattern: "FIXME"
    description: "Fix all FIXME items"

  - pattern: "HACK"
    description: "Remove hacky workarounds"

  - pattern: "panic\\("
    description: "Handle errors properly instead of panicking"

  - pattern: "console\\.log"
    description: "Remove debug logging"

  - pattern: "password\\s*=\\s*['\"][^'\"]+['\"]"
    description: "Do not hardcode passwords"
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `pattern` | string | Required | Regex pattern to match |
| `description` | string | No | Explanation shown in violations |

### Scoring

- Forbidden pattern found: **10 points** (High)

---

## Complexity Requirements

Ensure functions have minimum cyclomatic complexity (indicating real logic):

```yaml
complexity:
  - symbol: "ProcessRequest"
    file: "handler.go"
    min_complexity: 3

  - symbol: "ValidateInput"
    file: "validator.go"
    min_complexity: 5

  - symbol: "handleError"
    min_complexity: 2    # Search in any file
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `symbol` | string | Required | Function or method name |
| `file` | string | No | File to search (any file if omitted) |
| `min_complexity` | int | Required | Minimum cyclomatic complexity |

### Cyclomatic Complexity Calculation

Complexity starts at 1 and adds 1 for each:
- `if` statement
- `for` / `while` loop
- `case` in switch/match
- `&&` or `||` operator
- `?:` ternary operator
- `catch` clause

### Scoring

- Below minimum complexity: **10 points** (High)

---

## Required Tests

Verify that specific test functions exist:

```yaml
required_tests:
  - name: "TestProcessRequest"
    file: "handler_test.go"

  - name: "TestValidateInput"
    file: "validator_test.go"

  - name: "TestIntegration"
    # No file specified - searches all test files
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | Required | Test function name |
| `file` | string | No | Test file (any test file if omitted) |

### Scoring

- Missing required test: **5 points** (Low)

---

## Mock Signatures

Detect placeholder/mock data in code:

```yaml
mock_signatures:
  skip_test_files: true    # Don't flag mocks in test files

  patterns:
    # Domain patterns
    - pattern: 'example\.com'
      description: "Mock domain"

    - pattern: 'test\.local'
      description: "Test domain"

    # ID patterns
    - pattern: '\b(test|mock|fake|dummy)[-_]?(id|user|data)\b'
      description: "Mock identifier"

    # Placeholder text
    - pattern: 'lorem ipsum'
      description: "Placeholder text"

    - pattern: '\bfoo\b|\bbar\b|\bbaz\b'
      description: "Placeholder variable names"
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `skip_test_files` | bool | `true` | Exclude test files from mock detection |
| `patterns` | array | Required | List of mock patterns |
| `patterns[].pattern` | string | Required | Regex pattern |
| `patterns[].description` | string | No | Description of what it detects |

### Scoring

- Mock data found: **3 points** (Low)

---

## God Object Detection

Detect overly large files, functions, or classes:

```yaml
god_objects:
  enabled: true
  max_file_lines: 500
  max_function_lines: 50
  max_function_complexity: 15
  max_functions_per_file: 20
  max_class_methods: 15
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable god object detection |
| `max_file_lines` | int | `500` | Maximum lines per file |
| `max_function_lines` | int | `50` | Maximum lines per function |
| `max_function_complexity` | int | `15` | Maximum cyclomatic complexity |
| `max_functions_per_file` | int | `20` | Maximum functions per file |
| `max_class_methods` | int | `15` | Maximum methods per class |

### Scoring

- God object violation: **8 points** (Medium)

---

## Hollow TODO Detection

Detect TODO comments that lack meaningful context:

```yaml
hollow_todos:
  enabled: true
```

A TODO is considered "hollow" if it:
- Has no description after the TODO marker
- Contains only generic text like "implement this"
- Lacks actionable information

### Examples

```go
// Hollow TODOs (flagged):
// TODO
// TODO: implement
// TODO implement this

// Good TODOs (not flagged):
// TODO: Add rate limiting per RFC-123
// TODO(auth): Implement OAuth2 refresh token flow
// FIXME: Handle edge case when user has no permissions
```

### Scoring

- Hollow TODO found: **5 points** (Low)

---

## Dependency Verification

Verify that imported dependencies exist in package registries:

```yaml
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
    - "internal-*"        # Internal packages
    - "company-*"         # Company packages
    - "github.com/myorg/*"

  cache_ttl_hours: 24     # Cache registry lookups
  fail_on_timeout: false  # Don't fail if registry is unreachable
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `false` | Enable dependency verification |
| `registries` | object | All enabled | Which registries to check |
| `allowlist` | string[] | `[]` | Patterns to skip (glob syntax) |
| `cache_ttl_hours` | int | `24` | Cache duration for registry lookups |
| `fail_on_timeout` | bool | `false` | Fail if registry is unreachable |

### Supported Registries

| Registry | Languages | URL |
|----------|-----------|-----|
| `pypi` | Python | pypi.org |
| `npm` | JavaScript/TypeScript | npmjs.com |
| `crates` | Rust | crates.io |
| `go` | Go | proxy.golang.org |

### Scoring

- Hallucinated dependency: **15 points** (Critical)

---

## Scoring Reference

### Severity Levels and Points

| Severity | Points | Examples |
|----------|--------|----------|
| Critical | 15-20 | Missing files, missing symbols, hallucinated deps |
| High | 10 | Forbidden patterns, low complexity |
| Medium | 8 | God objects |
| Low | 3-5 | Mock data, missing tests, hollow TODOs |

### Grade Scale

| Grade | Score Range | Description |
|-------|-------------|-------------|
| A | 0-10 | Excellent quality |
| B | 11-25 | Good quality |
| C | 26-50 | Acceptable with issues |
| D | 51-75 | Poor quality |
| F | 76-100 | Failing quality |

---

## Complete Example

```yaml
version: "1.0"
name: "api-service"
description: "Quality contract for API service"
mode: "code"
include_test_files: false
excluded_paths:
  - "**/vendor/**"
  - "**/generated/**"
  - "**/mocks/**"

required_files:
  - path: "main.go"
    required: true
  - path: "README.md"
    required: true
  - path: "go.mod"
    required: true

required_symbols:
  - name: "main"
    kind: function
    file: "main.go"
  - name: "Server"
    kind: type
    file: "server.go"
  - name: "HandleRequest"
    kind: method
    file: "handler.go"

forbidden_patterns:
  - pattern: "TODO"
    description: "Remove TODO comments"
  - pattern: "panic\\("
    description: "Handle errors properly"
  - pattern: "fmt\\.Print"
    description: "Use structured logging"

complexity:
  - symbol: "HandleRequest"
    file: "handler.go"
    min_complexity: 3
  - symbol: "ValidateInput"
    file: "validator.go"
    min_complexity: 4

required_tests:
  - name: "TestHandleRequest"
    file: "handler_test.go"
  - name: "TestValidateInput"
    file: "validator_test.go"

mock_signatures:
  skip_test_files: true
  patterns:
    - pattern: 'example\.com'
      description: "Mock domain"
    - pattern: 'test[-_]?user'
      description: "Test user"

god_objects:
  enabled: true
  max_file_lines: 400
  max_function_lines: 40
  max_function_complexity: 12

hollow_todos:
  enabled: true

dependency_verification:
  enabled: true
  registries:
    go:
      enabled: true
  allowlist:
    - "github.com/myorg/*"

threshold: 25
```

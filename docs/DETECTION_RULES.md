# Detection Rules

Hollowcheck implements multiple detection rules to identify hollow, incomplete, or problematic code. This document details each detection rule and how it works.

## Overview

| Rule | Severity | Points | Description |
|------|----------|--------|-------------|
| Missing File | Critical | 20 | Required file doesn't exist |
| Hallucinated Dependency | Critical | 15 | Import doesn't exist in registry |
| Missing Symbol | Critical | 15 | Required function/type/const not found |
| Forbidden Pattern | High | 10 | Unwanted text pattern found |
| Low Complexity | High | 10 | Function below minimum complexity |
| God Object | Medium | 8 | Overly large file/function/class |
| Missing Test | Low | 5 | Required test function not found |
| Hollow TODO | Low | 5 | TODO without meaningful context |
| Mock Data | Low | 3 | Placeholder/mock data detected |
| Stub Function | High | 10 | Empty or trivial function body |

---

## Stub Function Detection

Detects functions that appear implemented but lack real logic.

### Detection Criteria

A function is flagged as a stub if it has:

1. **Empty Body**: No statements at all
   ```go
   func Process() {
   }
   ```

2. **Panic-Only**: Only panics/throws without logic
   ```go
   func Process() {
       panic("not implemented")
   }
   ```
   ```rust
   fn process() {
       unimplemented!()
   }
   ```
   ```python
   def process():
       raise NotImplementedError()
   ```

3. **Nil/Null Return Only**: Returns nil/null without logic
   ```go
   func GetUser() *User {
       return nil
   }
   ```
   ```typescript
   function getUser(): User | null {
       return null;
   }
   ```

4. **TODO Comment Only**: Contains only a TODO comment
   ```go
   func Process() {
       // TODO: implement this
   }
   ```

### Language-Specific Detection

| Language | Empty | Panic/Throw | Nil Return | TODO-Only |
|----------|-------|-------------|------------|-----------|
| Go | ✓ | `panic()` | `return nil` | ✓ |
| Rust | ✓ | `panic!`, `todo!`, `unimplemented!` | `None` | ✓ |
| Python | ✓ | `raise`, `NotImplementedError` | `return None` | ✓ |
| Java | ✓ | `throw` | `return null` | ✓ |
| TypeScript | ✓ | `throw` | `return null/undefined` | ✓ |
| JavaScript | ✓ | `throw` | `return null/undefined` | ✓ |
| C/C++ | ✓ | N/A | `return NULL/nullptr` | ✓ |
| Scala | ✓ | `throw`, `???` | `None`, `null` | ✓ |
| Swift | ✓ | `fatalError`, `preconditionFailure` | `return nil` | ✓ |

### Severity

- **High** (10 points) for stub functions

---

## Low Complexity Detection

Flags functions that don't meet minimum cyclomatic complexity requirements.

### Cyclomatic Complexity Calculation

Complexity = 1 + (decision points)

Decision points counted:
- `if` statements (+1 each)
- `for`, `while`, `do-while` loops (+1 each)
- `case` clauses in switch/match (+1 each)
- `&&` and `||` operators (+1 each)
- `?:` ternary operators (+1 each)
- `catch` clauses (+1 each)

### Example Complexity Calculations

```go
// Complexity: 1 (no decisions)
func simple() {
    return 42
}

// Complexity: 2 (1 if)
func hasIf(x int) int {
    if x > 0 {
        return x
    }
    return 0
}

// Complexity: 5 (1 if + 1 for + 1 if + 1 &&)
func complex(x int) int {
    if x > 0 {                    // +1
        for i := 0; i < x; i++ {  // +1
            if i%2 == 0 && i > 5 { // +1 +1
                return i
            }
        }
    }
    return 0
}
```

### Usage

```yaml
complexity:
  - symbol: "ProcessRequest"
    file: "handler.go"
    min_complexity: 3
```

### Severity

- **High** (10 points) when below minimum

---

## Forbidden Pattern Detection

Matches unwanted text patterns using regex.

### Common Patterns

```yaml
forbidden_patterns:
  # Work markers
  - pattern: "TODO"
  - pattern: "FIXME"
  - pattern: "HACK"
  - pattern: "XXX"

  # Debug code
  - pattern: "console\\.log"
  - pattern: "fmt\\.Print"
  - pattern: "print\\("
  - pattern: "debugger"

  # Security issues
  - pattern: "password\\s*=\\s*['\"][^'\"]+['\"]"
  - pattern: "api[_-]?key\\s*=\\s*['\"][^'\"]+['\"]"

  # Code smells
  - pattern: "panic\\("
  - pattern: "\\.unwrap\\(\\)"
  - pattern: "// nolint"
```

### Regex Syntax

Patterns use Rust regex syntax:
- `.` matches any character
- `\\.` matches literal dot
- `\\s` matches whitespace
- `\\b` matches word boundary
- `[...]` character class
- `(a|b)` alternation
- `*`, `+`, `?` quantifiers

### Severity

- **High** (10 points) per match

---

## Missing Symbol Detection

Verifies required functions, types, and constants exist.

### Symbol Kinds

| Kind | Matches |
|------|---------|
| `function` | Standalone functions |
| `method` | Methods on types/classes |
| `type` | Types, structs, classes, interfaces, enums |
| `const` | Constants |

### How It Works

1. Parse source files using tree-sitter
2. Extract declarations from AST
3. Match against required symbols by name and kind
4. Flag missing symbols

### Example

```yaml
required_symbols:
  - name: "ProcessRequest"
    kind: function
    file: "handler.go"
```

If `handler.go` doesn't contain a function named `ProcessRequest`, a violation is raised.

### Severity

- **Critical** (15 points) for missing symbols

---

## Hallucinated Dependency Detection

Detects imports that don't exist in package registries.

### Supported Registries

| Language | Registry | Detection Method |
|----------|----------|------------------|
| Python | PyPI | HTTP check to pypi.org |
| JavaScript/TypeScript | npm | HTTP check to npmjs.com |
| Rust | crates.io | HTTP check to crates.io |
| Go | Go Proxy | HTTP check to proxy.golang.org |

### How It Works

1. Extract imports from source files
2. Filter against allowlist patterns
3. Query package registries for existence
4. Cache results to avoid repeated lookups
5. Flag packages that return 404

### Configuration

```yaml
dependency_verification:
  enabled: true
  registries:
    pypi: { enabled: true }
    npm: { enabled: true }
  allowlist:
    - "internal-*"
    - "github.com/myorg/*"
  cache_ttl_hours: 24
  fail_on_timeout: false
```

### Severity

- **Critical** (15 points) for hallucinated dependencies

---

## God Object Detection

Identifies overly large or complex code structures.

### Checks

1. **File Size**: Files exceeding line limit
2. **Function Size**: Functions exceeding line limit
3. **Function Complexity**: Functions exceeding complexity limit
4. **Functions per File**: Too many functions in one file
5. **Methods per Class**: Classes with too many methods

### Default Thresholds

| Metric | Default Limit |
|--------|---------------|
| File lines | 500 |
| Function lines | 50 |
| Function complexity | 15 |
| Functions per file | 20 |
| Class methods | 15 |

### Configuration

```yaml
god_objects:
  enabled: true
  max_file_lines: 400
  max_function_lines: 40
  max_function_complexity: 12
  max_functions_per_file: 15
  max_class_methods: 10
```

### Severity

- **Medium** (8 points) per god object violation

---

## Mock Data Detection

Identifies placeholder and mock data in production code.

### Common Patterns

```yaml
mock_signatures:
  skip_test_files: true
  patterns:
    # Domains
    - pattern: 'example\.(com|org|net)'
    - pattern: 'test\.local'
    - pattern: 'localhost'

    # IDs
    - pattern: '\b(test|mock|fake|dummy)[-_]?(id|user|data)\b'
    - pattern: '\b(12345|11111|00000)\b'

    # Placeholder text
    - pattern: 'lorem ipsum'
    - pattern: 'foo|bar|baz'
    - pattern: 'asdf|qwerty'

    # Emails
    - pattern: 'test@example\.com'
    - pattern: 'user@test\.com'
```

### Test File Handling

By default, mock data in test files is not flagged:

```yaml
mock_signatures:
  skip_test_files: true  # Don't flag in *_test.go, test_*.py, etc.
```

### Severity

- **Low** (3 points) per mock data instance

---

## Hollow TODO Detection

Identifies TODO comments lacking meaningful context.

### Hollow TODO Patterns

```go
// Hollow (flagged):
// TODO
// TODO:
// TODO: implement
// TODO implement this
// FIXME

// Not Hollow (not flagged):
// TODO: Add rate limiting per RFC-123
// TODO(auth): Implement OAuth2 refresh token flow
// FIXME: Handle edge case when user count exceeds 1000
// TODO @john: Review security implications of this change
```

### Detection Logic

A TODO is considered hollow if:
1. Has no text after the marker
2. Contains only generic words like "implement", "fix", "this", "later"
3. Lacks specific context, references, or actionable information

### Severity

- **Low** (5 points) per hollow TODO

---

## Missing Test Detection

Verifies required test functions exist.

### Test File Patterns

| Language | Test File Pattern |
|----------|-------------------|
| Go | `*_test.go` |
| Python | `test_*.py`, `*_test.py` |
| JavaScript | `*.test.js`, `*.spec.js` |
| TypeScript | `*.test.ts`, `*.spec.ts` |
| Rust | Inline `#[test]` or `tests/` directory |
| Java | `*Test.java` |

### Test Function Patterns

| Language | Test Function Pattern |
|----------|----------------------|
| Go | `func Test*` |
| Python | `def test_*` |
| JavaScript/TypeScript | `it()`, `test()`, `describe()` |
| Rust | `#[test] fn` |
| Java | `@Test void` |

### Configuration

```yaml
required_tests:
  - name: "TestProcessRequest"
    file: "handler_test.go"

  - name: "test_validate_input"
    file: "test_validator.py"
```

### Severity

- **Low** (5 points) per missing test

---

## Missing File Detection

Verifies required files exist in the codebase.

### Configuration

```yaml
required_files:
  - path: "main.go"
    required: true    # Critical if missing

  - path: "README.md"
    required: true

  - path: "docs/API.md"
    required: false   # Warning only
```

### Severity

- **Critical** (20 points) for required files
- **Low** (5 points) for optional files

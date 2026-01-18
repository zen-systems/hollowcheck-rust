//! Integration tests for tree-sitter parser infrastructure.
//!
//! These tests validate symbol extraction and complexity calculation
//! against real source files and testdata fixtures.

use hollowcheck::parser;

/// Initialize parsers before running tests.
fn setup() {
    parser::init();
}

// =============================================================================
// Go Parser Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_go_symbol_extraction() {
    setup();

    let source = br#"
package main

const Version = "1.0"

type Config struct {
    Name string
}

func (c *Config) Validate() error {
    return nil
}

func main() {
    fmt.Println("hello")
}

func helper() int {
    return 42
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    // Check we found the expected symbols
    assert!(
        symbols.iter().any(|s| s.name == "Version" && s.kind == "const"),
        "Expected Version const"
    );
    assert!(
        symbols.iter().any(|s| s.name == "Config" && s.kind == "type"),
        "Expected Config type"
    );
    assert!(
        symbols.iter().any(|s| s.name == "Validate" && s.kind == "method"),
        "Expected Validate method"
    );
    assert!(
        symbols.iter().any(|s| s.name == "main" && s.kind == "function"),
        "Expected main function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "helper" && s.kind == "function"),
        "Expected helper function"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_go_complexity_simple() {
    setup();

    let source = br#"
package main

func simple() int {
    return 42
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let complexity = parser.complexity(source, "simple").expect("should calculate complexity");

    assert_eq!(complexity, 1, "Simple function should have complexity 1");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_go_complexity_with_branches() {
    setup();

    let source = br#"
package main

func branchy(x int) int {
    if x > 0 {
        return 1
    } else if x < 0 {
        return -1
    }
    return 0
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let complexity = parser.complexity(source, "branchy").expect("should calculate complexity");

    // 1 (base) + 2 (if statements)
    assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_go_complexity_with_loops_and_boolean() {
    setup();

    let source = br#"
package main

func complex(x int) bool {
    for i := 0; i < x; i++ {
        if i > 5 && i < 10 {
            return true
        }
    }
    return false
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let complexity = parser.complexity(source, "complex").expect("should calculate complexity");

    // 1 (base) + 1 (for) + 1 (if) + 1 (&&) = 4
    assert!(complexity >= 4, "Expected >= 4, got {}", complexity);
}

// =============================================================================
// Python Parser Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_python_symbol_extraction() {
    setup();

    let source = br#"
class MyClass:
    def __init__(self, name):
        self.name = name

    def greet(self):
        return f"Hello, {self.name}"

def standalone_function(x):
    return x * 2

async def async_function():
    pass
"#;

    let parser = parser::for_extension(".py").expect("Python parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    assert!(
        symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
        "Expected MyClass"
    );
    // Python parser marks all function definitions as "function" (including methods)
    assert!(
        symbols.iter().any(|s| s.name == "__init__" && s.kind == "function"),
        "Expected __init__ function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "greet" && s.kind == "function"),
        "Expected greet function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "standalone_function" && s.kind == "function"),
        "Expected standalone_function"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_python_complexity_simple() {
    setup();

    let source = br#"
def simple():
    return 42
"#;

    let parser = parser::for_extension(".py").expect("Python parser should be available");
    let complexity = parser.complexity(source, "simple").expect("should calculate complexity");

    assert_eq!(complexity, 1, "Simple function should have complexity 1");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_python_complexity_with_branches() {
    setup();

    let source = br#"
def branchy(x):
    if x > 0:
        return 1
    elif x < 0:
        return -1
    else:
        return 0
"#;

    let parser = parser::for_extension(".py").expect("Python parser should be available");
    let complexity = parser.complexity(source, "branchy").expect("should calculate complexity");

    // 1 (base) + 1 (if) + 1 (elif)
    assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_python_complexity_with_loops() {
    setup();

    let source = br#"
def loopy(items):
    result = []
    for item in items:
        while item > 0:
            result.append(item)
            item -= 1
    return result
"#;

    let parser = parser::for_extension(".py").expect("Python parser should be available");
    let complexity = parser.complexity(source, "loopy").expect("should calculate complexity");

    // 1 (base) + 1 (for) + 1 (while)
    assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
}

// =============================================================================
// TypeScript Parser Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_typescript_symbol_extraction() {
    setup();

    let source = br#"
function hello(): void {
    console.log("hello");
}

class MyClass {
    method(): number {
        return 42;
    }
}

interface IConfig {
    name: string;
}

type StringArray = string[];
"#;

    let parser = parser::for_extension(".ts").expect("TypeScript parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    assert!(
        symbols.iter().any(|s| s.name == "hello" && s.kind == "function"),
        "Expected hello function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
        "Expected MyClass"
    );
    assert!(
        symbols.iter().any(|s| s.name == "method" && s.kind == "method"),
        "Expected method"
    );
    assert!(
        symbols.iter().any(|s| s.name == "IConfig" && s.kind == "type"),
        "Expected IConfig interface"
    );
    assert!(
        symbols.iter().any(|s| s.name == "StringArray" && s.kind == "type"),
        "Expected StringArray type"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_typescript_complexity_simple() {
    setup();

    let source = br#"
function simple(): number {
    return 42;
}
"#;

    let parser = parser::for_extension(".ts").expect("TypeScript parser should be available");
    let complexity = parser.complexity(source, "simple").expect("should calculate complexity");

    assert_eq!(complexity, 1, "Simple function should have complexity 1");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_typescript_complexity_with_branches() {
    setup();

    let source = br#"
function branchy(x: number): number {
    if (x > 0) {
        return 1;
    } else if (x < 0) {
        return -1;
    }
    return 0;
}
"#;

    let parser = parser::for_extension(".ts").expect("TypeScript parser should be available");
    let complexity = parser.complexity(source, "branchy").expect("should calculate complexity");

    // 1 (base) + 2 (if statements)
    assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_javascript_extension() {
    setup();

    let source = br#"
function add(a, b) {
    return a + b;
}
"#;

    let parser = parser::for_extension(".js").expect("JavaScript parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    assert!(
        symbols.iter().any(|s| s.name == "add" && s.kind == "function"),
        "Expected add function"
    );
}

// =============================================================================
// Java Parser Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_java_symbol_extraction() {
    setup();

    let source = br#"
public class MyClass {
    public void method() {
        System.out.println("hello");
    }

    public int calculate(int x) {
        return x * 2;
    }
}

interface MyInterface {
    void doSomething();
}

enum Status {
    ACTIVE,
    INACTIVE
}
"#;

    let parser = parser::for_extension(".java").expect("Java parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    assert!(
        symbols.iter().any(|s| s.name == "MyClass" && s.kind == "type"),
        "Expected MyClass"
    );
    assert!(
        symbols.iter().any(|s| s.name == "method" && s.kind == "method"),
        "Expected method"
    );
    assert!(
        symbols.iter().any(|s| s.name == "calculate" && s.kind == "method"),
        "Expected calculate"
    );
    assert!(
        symbols.iter().any(|s| s.name == "MyInterface" && s.kind == "type"),
        "Expected MyInterface"
    );
    assert!(
        symbols.iter().any(|s| s.name == "Status" && s.kind == "type"),
        "Expected Status enum"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_java_complexity_simple() {
    setup();

    let source = br#"
public class Test {
    public int simple() {
        return 42;
    }
}
"#;

    let parser = parser::for_extension(".java").expect("Java parser should be available");
    let complexity = parser.complexity(source, "simple").expect("should calculate complexity");

    assert_eq!(complexity, 1, "Simple method should have complexity 1");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_java_complexity_with_branches() {
    setup();

    let source = br#"
public class Test {
    public int branchy(int x) {
        if (x > 0) {
            return 1;
        } else if (x < 0) {
            return -1;
        } else {
            return 0;
        }
    }
}
"#;

    let parser = parser::for_extension(".java").expect("Java parser should be available");
    let complexity = parser.complexity(source, "branchy").expect("should calculate complexity");

    // 1 (base) + 2 (if statements)
    assert!(complexity >= 3, "Expected >= 3, got {}", complexity);
}

// =============================================================================
// Testdata Integration Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_testdata_clean_go_symbols() {
    setup();

    let source = include_bytes!("../testdata/clean.go");
    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    // Check key symbols from clean.go
    assert!(
        symbols.iter().any(|s| s.name == "Config" && s.kind == "type"),
        "Expected Config type"
    );
    assert!(
        symbols.iter().any(|s| s.name == "MaxConnections" && s.kind == "const"),
        "Expected MaxConnections const"
    );
    assert!(
        symbols.iter().any(|s| s.name == "Validate" && s.kind == "method"),
        "Expected Validate method"
    );
    assert!(
        symbols.iter().any(|s| s.name == "ProcessItems" && s.kind == "function"),
        "Expected ProcessItems function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "CalculateScore" && s.kind == "function"),
        "Expected CalculateScore function"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_testdata_clean_go_complexity() {
    setup();

    let source = include_bytes!("../testdata/clean.go");
    let parser = parser::for_extension(".go").expect("Go parser should be available");

    // ProcessItems has significant complexity (for loop, multiple if statements, && operator)
    let process_items_complexity = parser
        .complexity(source, "ProcessItems")
        .expect("should calculate complexity");
    assert!(
        process_items_complexity >= 5,
        "ProcessItems should have complexity >= 5, got {}",
        process_items_complexity
    );

    // CalculateScore also has significant complexity
    let calculate_score_complexity = parser
        .complexity(source, "CalculateScore")
        .expect("should calculate complexity");
    assert!(
        calculate_score_complexity >= 5,
        "CalculateScore should have complexity >= 5, got {}",
        calculate_score_complexity
    );

    // Validate has moderate complexity (multiple if statements)
    let validate_complexity = parser
        .complexity(source, "Validate")
        .expect("should calculate complexity");
    assert!(
        validate_complexity >= 4,
        "Validate should have complexity >= 4, got {}",
        validate_complexity
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_testdata_stub_go_symbols() {
    setup();

    let source = include_bytes!("../testdata/stub.go");
    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    // Check key symbols from stub.go
    assert!(
        symbols.iter().any(|s| s.name == "StubConfig" && s.kind == "type"),
        "Expected StubConfig type"
    );
    assert!(
        symbols.iter().any(|s| s.name == "DefaultTimeout" && s.kind == "const"),
        "Expected DefaultTimeout const"
    );
    assert!(
        symbols.iter().any(|s| s.name == "ProcessData" && s.kind == "function"),
        "Expected ProcessData function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "ValidateInput" && s.kind == "function"),
        "Expected ValidateInput function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "HandleRequest" && s.kind == "function"),
        "Expected HandleRequest function"
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_testdata_stub_go_low_complexity() {
    setup();

    let source = include_bytes!("../testdata/stub.go");
    let parser = parser::for_extension(".go").expect("Go parser should be available");

    // ProcessData is a stub with minimal complexity
    let process_data_complexity = parser
        .complexity(source, "ProcessData")
        .expect("should calculate complexity");
    assert!(
        process_data_complexity < 3,
        "ProcessData (stub) should have low complexity < 3, got {}",
        process_data_complexity
    );

    // ValidateInput is also a stub
    let validate_input_complexity = parser
        .complexity(source, "ValidateInput")
        .expect("should calculate complexity");
    assert!(
        validate_input_complexity < 3,
        "ValidateInput (stub) should have low complexity < 3, got {}",
        validate_input_complexity
    );
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_testdata_mock_go_symbols() {
    setup();

    let source = include_bytes!("../testdata/mock.go");
    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let symbols = parser.parse_symbols(source).expect("should parse symbols");

    // Check key symbols from mock.go
    assert!(
        symbols.iter().any(|s| s.name == "MockUser" && s.kind == "type"),
        "Expected MockUser type"
    );
    assert!(
        symbols.iter().any(|s| s.name == "MockConfig" && s.kind == "type"),
        "Expected MockConfig type"
    );
    assert!(
        symbols.iter().any(|s| s.name == "GetTestUser" && s.kind == "function"),
        "Expected GetTestUser function"
    );
    assert!(
        symbols.iter().any(|s| s.name == "GetDescription" && s.kind == "function"),
        "Expected GetDescription function"
    );
}

// =============================================================================
// Parser Registry Tests
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_parser_registry_extensions() {
    setup();

    // All these extensions should have parsers
    let extensions = [".go", ".py", ".ts", ".tsx", ".js", ".jsx", ".java"];

    for ext in extensions {
        assert!(
            parser::for_extension(ext).is_some(),
            "Parser for {} should be available",
            ext
        );
    }
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_parser_registry_unknown_extension() {
    setup();

    // Unknown extensions should return None
    let unknown = parser::for_extension(".unknown");
    assert!(unknown.is_none(), "Unknown extension should return None");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_parser_language_names() {
    setup();

    let cases = [
        (".go", "go"),
        (".py", "python"),
        (".ts", "typescript"),
        (".js", "javascript"),
        (".java", "java"),
    ];

    for (ext, expected_lang) in cases {
        let parser = parser::for_extension(ext).expect(&format!("Parser for {} should exist", ext));
        assert_eq!(
            parser.language(),
            expected_lang,
            "Parser for {} should report language as {}",
            ext,
            expected_lang
        );
    }
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

#[test]
#[cfg(feature = "tree-sitter")]
fn test_complexity_nonexistent_function() {
    setup();

    let source = br#"
package main

func existing() {
    return
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");
    let complexity = parser
        .complexity(source, "nonexistent")
        .expect("should handle missing function");

    assert_eq!(complexity, 0, "Missing function should return complexity 0");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_empty_source() {
    setup();

    let source = b"";
    let parser = parser::for_extension(".go").expect("Go parser should be available");

    let symbols = parser.parse_symbols(source).expect("should handle empty source");
    assert!(symbols.is_empty(), "Empty source should have no symbols");
}

#[test]
#[cfg(feature = "tree-sitter")]
fn test_source_with_syntax_errors() {
    setup();

    // Malformed Go code
    let source = br#"
package main

func broken( {
    if
}
"#;

    let parser = parser::for_extension(".go").expect("Go parser should be available");

    // Tree-sitter is generally resilient to syntax errors
    // It should still be able to parse something
    let result = parser.parse_symbols(source);
    assert!(result.is_ok(), "Parser should handle syntax errors gracefully");
}

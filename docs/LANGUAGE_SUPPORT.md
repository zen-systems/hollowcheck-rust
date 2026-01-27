# Language Support

Hollowcheck uses tree-sitter for AST-based code analysis. This document describes supported languages and their implementation status.

## Supported Languages

| Language | Extensions | Status | Stub Detection | Complexity | Imports |
|----------|------------|--------|----------------|------------|---------|
| Go | `.go` | ✅ Complete | ✅ | ✅ | ✅ |
| Rust | `.rs` | ✅ Complete | ✅ | ✅ | ✅ |
| Python | `.py` | ⚠️ Partial | ✅ | ✅ | ❌ |
| Java | `.java` | ⚠️ Partial | ✅ | ✅ | ❌ |
| TypeScript | `.ts`, `.tsx`, `.mts` | ⚠️ Partial | ✅ | ✅ | ❌ |
| JavaScript | `.js`, `.jsx`, `.mjs` | ⚠️ Partial | ✅ | ✅ | ❌ |
| C | `.c`, `.h` | ⚠️ Partial | ✅ | ✅ | ❌ |
| C++ | `.cpp`, `.cc`, `.hpp` | ⚠️ Partial | ✅ | ✅ | ❌ |
| Scala | `.scala`, `.sc` | ⚠️ Partial | ✅ | ✅ | ❌ |
| Swift | `.swift` | ⚠️ Partial | ✅ | ✅ | ❌ |

**Legend:**
- ✅ Complete: Full implementation with tests
- ⚠️ Partial: Core functionality works, missing import extraction and/or tests
- ❌ Not implemented

---

## Feature Matrix

### Declaration Extraction

All analyzers extract these declaration types:

| Declaration | Go | Rust | Python | Java | TS | JS | C | C++ | Scala | Swift |
|-------------|----|----|--------|------|----|----|---|-----|-------|-------|
| Functions | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Methods | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |
| Structs/Classes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Interfaces/Traits | ✅ | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| Enums | ❌ | ✅ | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ | ❌ | ✅ |
| Constants | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

### Control Flow Analysis

All analyzers count these constructs for complexity:

| Construct | Go | Rust | Python | Java | TS | JS | C | C++ | Scala | Swift |
|-----------|----|----|--------|------|----|----|---|-----|-------|-------|
| `if` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `for`/`while` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `switch`/`match` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `case` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `&&`/`\|\|` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Ternary `?:` | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| `try`/`catch` | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |

### Stub Detection Patterns

| Pattern | Go | Rust | Python | Java | TS | JS | C | C++ | Scala | Swift |
|---------|----|----|--------|------|----|----|---|-----|-------|-------|
| Empty body | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Panic only | `panic()` | `panic!`, `todo!`, `unimplemented!` | `raise` | `throw` | `throw` | `throw` | ❌ | ❌ | `throw`, `???` | `fatalError` |
| Nil return | `return nil` | `None` | `return None` | `return null` | `return null/undefined` | `return null/undefined` | `return NULL` | `return nullptr` | `None`, `null` | `return nil` |
| TODO only | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

---

## Language-Specific Details

### Go

**File Extensions:** `.go`

**Declarations Extracted:**
- Functions (`func name()`)
- Methods (`func (r *Receiver) name()`)
- Types (`type Name struct/interface`)
- Constants (`const Name = value`)

**Stub Patterns:**
- `panic("...")` or `panic(errors.New(...))`
- `return nil`
- Empty function body
- TODO-only comment

**Import Extraction:** ✅ Supported
- Single imports: `import "path"`
- Grouped imports: `import (...)`

### Rust

**File Extensions:** `.rs`

**Declarations Extracted:**
- Functions (`fn name()`)
- Methods (`impl Type { fn name() }`)
- Structs (`struct Name`)
- Enums (`enum Name`)
- Traits (`trait Name`)
- Constants (`const NAME: Type = value`)

**Stub Patterns:**
- `panic!("...")`, `todo!()`, `unimplemented!()`
- `None` return
- Empty function body
- TODO-only comment

**Import Extraction:** ✅ Supported
- `use path::to::item`
- `use path::{item1, item2}`

### Python

**File Extensions:** `.py`

**Declarations Extracted:**
- Functions (`def name():`)
- Methods (`def name(self):`)
- Classes (`class Name:`)
- Constants (UPPER_CASE assignments)

**Stub Patterns:**
- `raise NotImplementedError()`
- `raise Exception("not implemented")`
- `return None`
- `pass` only
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### Java

**File Extensions:** `.java`

**Declarations Extracted:**
- Methods (instance and static)
- Classes (`class Name`)
- Interfaces (`interface Name`)
- Enums (`enum Name`)
- Constants (`static final`)

**Stub Patterns:**
- `throw new UnsupportedOperationException()`
- `throw new RuntimeException("not implemented")`
- `return null`
- Empty method body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### TypeScript

**File Extensions:** `.ts`, `.tsx`, `.mts`

**Declarations Extracted:**
- Functions (`function name()`)
- Arrow functions (`const name = () => {}`)
- Methods (class methods)
- Classes (`class Name`)
- Interfaces (`interface Name`)
- Type aliases (`type Name = ...`)
- Enums (`enum Name`)

**Stub Patterns:**
- `throw new Error("not implemented")`
- `return null` or `return undefined`
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### JavaScript

**File Extensions:** `.js`, `.jsx`, `.mjs`

**Declarations Extracted:**
- Functions (`function name()`)
- Arrow functions (`const name = () => {}`)
- Methods (class/object methods)
- Classes (`class Name`)

**Stub Patterns:**
- `throw new Error("not implemented")`
- `return null` or `return undefined`
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### C

**File Extensions:** `.c`, `.h`

**Declarations Extracted:**
- Functions
- Structs (`struct name`)
- Enums (`enum name`)
- Constants (`#define`, `const`)

**Stub Patterns:**
- `return NULL`
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### C++

**File Extensions:** `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`

**Declarations Extracted:**
- Functions
- Methods (class members)
- Classes (`class Name`)
- Structs (`struct Name`)
- Enums (`enum Name`, `enum class Name`)
- Constants (`const`, `constexpr`)

**Stub Patterns:**
- `throw std::runtime_error("not implemented")`
- `return nullptr` or `return NULL`
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### Scala

**File Extensions:** `.scala`, `.sc`

**Declarations Extracted:**
- Functions (`def name()`)
- Methods (class/object methods)
- Classes (`class Name`)
- Objects (`object Name`)
- Traits (`trait Name`)
- Constants (`val NAME`)

**Stub Patterns:**
- `???` (Scala's "not implemented" marker)
- `throw new NotImplementedError()`
- `None` or `null` return
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

### Swift

**File Extensions:** `.swift`

**Declarations Extracted:**
- Functions (`func name()`)
- Methods (class/struct methods)
- Classes (`class Name`)
- Structs (`struct Name`)
- Enums (`enum Name`)
- Protocols (`protocol Name`)
- Constants (`let name`)

**Stub Patterns:**
- `fatalError("not implemented")`
- `preconditionFailure("...")`
- `return nil`
- Empty function body
- TODO-only comment

**Import Extraction:** ❌ Not yet implemented

---

## Implementation Roadmap

### Completed

1. ✅ Go analyzer (full implementation + tests)
2. ✅ Rust analyzer (full implementation + tests)
3. ✅ Core detection for all 10 languages

### In Progress

4. ⏳ Import extraction for remaining languages
5. ⏳ Unit tests for remaining analyzers

### Planned

6. 📋 Ruby analyzer
7. 📋 PHP analyzer
8. 📋 Kotlin analyzer
9. 📋 C# analyzer

---

## Adding a New Language

To add support for a new language:

1. **Add tree-sitter grammar** to `Cargo.toml`:
   ```toml
   [dependencies]
   tree-sitter-newlang = "0.x"
   ```

2. **Create analyzer file** at `src/analysis/languages/newlang.rs`:
   ```rust
   use tree_sitter::{Language, Parser, Query};
   use crate::analysis::{LanguageAnalyzer, ParsedFile, FileFacts};

   const DECLARATION_QUERY: &str = r#"
   ; Function declarations
   (function_definition name: (identifier) @func_name) @function
   "#;

   const CONTROL_FLOW_QUERY: &str = r#"
   (if_statement) @if
   (for_statement) @for
   "#;

   pub struct NewLangAnalyzer {
       language: Language,
   }

   impl LanguageAnalyzer for NewLangAnalyzer {
       fn language_id(&self) -> &'static str { "newlang" }
       fn file_extensions(&self) -> &'static [&'static str] { &["nl"] }
       // ... implement remaining methods
   }
   ```

3. **Register in mod.rs**:
   ```rust
   pub mod newlang;

   pub fn register_analyzers() {
       // ...
       register_analyzer(Box::new(newlang::NewLangAnalyzer::new()));
   }
   ```

4. **Add tests** in the analyzer file

5. **Update documentation** in this file

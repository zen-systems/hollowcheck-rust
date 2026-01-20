//! Import extraction from source files.
//!
//! Extracts package imports using tree-sitter parsers and regex fallback.
//! Supports Python, JavaScript/TypeScript, Go, and Rust.

use super::stdlib::{is_stdlib, StdlibLanguage};
use crate::registry::RegistryType;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Information about an imported dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImportedDependency {
    /// The package/module name
    pub name: String,
    /// Which registry this dependency belongs to
    pub registry: RegistryType,
    /// Source file where the import was found
    pub file: String,
    /// Line number where the import was found
    pub line: usize,
}

/// Extract all imports from a source file.
pub fn extract_imports(file_path: &Path) -> anyhow::Result<Vec<ImportedDependency>> {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let content = fs::read_to_string(file_path)?;
    let file_str = file_path.to_string_lossy().to_string();

    let registry = match RegistryType::from_extension(ext) {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let imports = match registry {
        RegistryType::PyPI => extract_python_imports(&content, &file_str),
        RegistryType::Npm => extract_js_imports(&content, &file_str),
        RegistryType::Go => extract_go_imports(&content, &file_str),
        RegistryType::Crates => extract_rust_imports(&content, &file_str),
    };

    Ok(imports)
}

/// Extract imports from Python source code.
fn extract_python_imports(content: &str, file: &str) -> Vec<ImportedDependency> {
    lazy_static::lazy_static! {
        // import foo, bar (must be at start of line, possibly with indentation)
        static ref IMPORT_RE: Regex = Regex::new(r"^\s*import\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        // from foo import bar (must have 'import' keyword after module name)
        static ref FROM_IMPORT_RE: Regex = Regex::new(r"^\s*from\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+import\b").unwrap();
    }

    let mut imports = Vec::new();
    let mut seen = HashSet::new();
    let mut in_docstring = false;
    let mut docstring_char = '"';

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Track docstring state (triple-quoted strings)
        if !in_docstring {
            if trimmed.starts_with(r#"""""#) || trimmed.starts_with("'''") {
                docstring_char = if trimmed.starts_with('"') { '"' } else { '\'' };
                // Check if docstring ends on same line
                let rest = &trimmed[3..];
                let end_pattern = if docstring_char == '"' { r#"""""# } else { "'''" };
                if !rest.contains(end_pattern) {
                    in_docstring = true;
                }
                continue;
            }
        } else {
            let end_pattern = if docstring_char == '"' { r#"""""# } else { "'''" };
            if trimmed.contains(end_pattern) {
                in_docstring = false;
            }
            continue;
        }

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // import foo
        if let Some(caps) = IMPORT_RE.captures(line) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if is_valid_python_import(&name) && seen.insert(name.clone()) {
                imports.push(ImportedDependency {
                    name,
                    registry: RegistryType::PyPI,
                    file: file.to_string(),
                    line: line_num + 1,
                });
            }
        }

        // from foo import bar
        if let Some(caps) = FROM_IMPORT_RE.captures(line) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if is_valid_python_import(&name) && seen.insert(name.clone()) {
                imports.push(ImportedDependency {
                    name,
                    registry: RegistryType::PyPI,
                    file: file.to_string(),
                    line: line_num + 1,
                });
            }
        }
    }

    imports
}

/// Check if a Python module name is a valid external import to check.
fn is_valid_python_import(name: &str) -> bool {
    // Skip stdlib
    if is_stdlib(StdlibLanguage::Python, name) {
        return false;
    }

    // Skip private/internal modules (start with underscore)
    if name.starts_with('_') {
        return false;
    }

    // Skip relative imports (though these shouldn't match our regex)
    if name.starts_with('.') {
        return false;
    }

    true
}

/// Extract imports from JavaScript/TypeScript source code.
fn extract_js_imports(content: &str, file: &str) -> Vec<ImportedDependency> {
    lazy_static::lazy_static! {
        // import x from 'package' or import 'package'
        static ref IMPORT_RE: Regex = Regex::new(r#"(?m)^(?:import\s+(?:[\w{},\s*]+\s+from\s+)?['"]([^'"./][^'"]*?)['"]|import\s*\(['"]([^'"./][^'"]*?)['"]\))"#).unwrap();
        // require('package')
        static ref REQUIRE_RE: Regex = Regex::new(r#"require\s*\(\s*['"]([^'"./][^'"]*?)['"]\s*\)"#).unwrap();
    }

    let mut imports = Vec::new();
    let mut seen = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        // ES6 imports
        for caps in IMPORT_RE.captures_iter(trimmed) {
            let name = caps.get(1).or_else(|| caps.get(2));
            if let Some(m) = name {
                let pkg = extract_npm_package_name(m.as_str());
                if !is_stdlib(StdlibLanguage::JavaScript, &pkg) && seen.insert(pkg.clone()) {
                    imports.push(ImportedDependency {
                        name: pkg,
                        registry: RegistryType::Npm,
                        file: file.to_string(),
                        line: line_num + 1,
                    });
                }
            }
        }

        // CommonJS require
        for caps in REQUIRE_RE.captures_iter(line) {
            if let Some(m) = caps.get(1) {
                let pkg = extract_npm_package_name(m.as_str());
                if !is_stdlib(StdlibLanguage::JavaScript, &pkg) && seen.insert(pkg.clone()) {
                    imports.push(ImportedDependency {
                        name: pkg,
                        registry: RegistryType::Npm,
                        file: file.to_string(),
                        line: line_num + 1,
                    });
                }
            }
        }
    }

    imports
}

/// Extract the package name from an npm import path.
/// For scoped packages (@org/pkg/...), returns @org/pkg.
/// For regular packages (pkg/...), returns pkg.
fn extract_npm_package_name(import_path: &str) -> String {
    if import_path.starts_with('@') {
        // Scoped package: @org/pkg/subpath -> @org/pkg
        let parts: Vec<&str> = import_path.splitn(3, '/').collect();
        if parts.len() >= 2 {
            format!("{}/{}", parts[0], parts[1])
        } else {
            import_path.to_string()
        }
    } else {
        // Regular package: pkg/subpath -> pkg
        import_path
            .split('/')
            .next()
            .unwrap_or(import_path)
            .to_string()
    }
}

/// Extract imports from Go source code.
fn extract_go_imports(content: &str, file: &str) -> Vec<ImportedDependency> {
    lazy_static::lazy_static! {
        // Single import: import "package"
        static ref SINGLE_IMPORT_RE: Regex = Regex::new(r#"(?m)^import\s+"([^"]+)""#).unwrap();
        // Import block: import ( "pkg1" "pkg2" )
        static ref IMPORT_BLOCK_RE: Regex = Regex::new(r#"(?s)import\s*\((.*?)\)"#).unwrap();
        // Individual import within block
        static ref BLOCK_ITEM_RE: Regex = Regex::new(r#"(?:_\s+)?"([^"]+)""#).unwrap();
    }

    let mut imports = Vec::new();
    let mut seen = HashSet::new();

    // Find line numbers for all imports
    let line_map: std::collections::HashMap<&str, usize> = content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            if line.contains('"') {
                // Try to extract the import path
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let path = &line[start + 1..start + 1 + end];
                        return Some((path, i + 1));
                    }
                }
            }
            None
        })
        .collect();

    // Process single imports
    for caps in SINGLE_IMPORT_RE.captures_iter(content) {
        if let Some(m) = caps.get(1) {
            let import_path = m.as_str();
            if !is_stdlib(StdlibLanguage::Go, import_path) {
                let pkg = extract_go_module_name(import_path);
                if seen.insert(pkg.clone()) {
                    let line = line_map.get(import_path).copied().unwrap_or(1);
                    imports.push(ImportedDependency {
                        name: pkg,
                        registry: RegistryType::Go,
                        file: file.to_string(),
                        line,
                    });
                }
            }
        }
    }

    // Process import blocks
    for block_caps in IMPORT_BLOCK_RE.captures_iter(content) {
        if let Some(block) = block_caps.get(1) {
            for caps in BLOCK_ITEM_RE.captures_iter(block.as_str()) {
                if let Some(m) = caps.get(1) {
                    let import_path = m.as_str();
                    if !is_stdlib(StdlibLanguage::Go, import_path) {
                        let pkg = extract_go_module_name(import_path);
                        if seen.insert(pkg.clone()) {
                            let line = line_map.get(import_path).copied().unwrap_or(1);
                            imports.push(ImportedDependency {
                                name: pkg,
                                registry: RegistryType::Go,
                                file: file.to_string(),
                                line,
                            });
                        }
                    }
                }
            }
        }
    }

    imports
}

/// Extract the module name from a Go import path.
/// github.com/user/repo/pkg -> github.com/user/repo
fn extract_go_module_name(import_path: &str) -> String {
    let parts: Vec<&str> = import_path.split('/').collect();

    // Most Go modules are at least 3 parts: domain/user/repo
    if parts.len() >= 3 {
        format!("{}/{}/{}", parts[0], parts[1], parts[2])
    } else {
        import_path.to_string()
    }
}

/// Extract imports from Rust source code.
fn extract_rust_imports(content: &str, file: &str) -> Vec<ImportedDependency> {
    lazy_static::lazy_static! {
        // use crate_name::...
        static ref USE_RE: Regex = Regex::new(r"(?m)^use\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:::|;)").unwrap();
        // extern crate crate_name
        static ref EXTERN_CRATE_RE: Regex = Regex::new(r"(?m)^extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    }

    let mut imports = Vec::new();
    let mut seen = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        // use statements
        if let Some(caps) = USE_RE.captures(trimmed) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if !is_stdlib(StdlibLanguage::Rust, &name) && seen.insert(name.clone()) {
                imports.push(ImportedDependency {
                    name,
                    registry: RegistryType::Crates,
                    file: file.to_string(),
                    line: line_num + 1,
                });
            }
        }

        // extern crate
        if let Some(caps) = EXTERN_CRATE_RE.captures(trimmed) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if !is_stdlib(StdlibLanguage::Rust, &name) && seen.insert(name.clone()) {
                imports.push(ImportedDependency {
                    name,
                    registry: RegistryType::Crates,
                    file: file.to_string(),
                    line: line_num + 1,
                });
            }
        }
    }

    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_imports() {
        let content = r#"
import requests
import json
from flask import Flask
from typing import Optional
"#;
        let imports = extract_python_imports(content, "test.py");

        let names: Vec<&str> = imports.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"requests"));
        assert!(names.contains(&"flask"));
        // json and typing are stdlib
        assert!(!names.contains(&"json"));
        assert!(!names.contains(&"typing"));
    }

    #[test]
    fn test_extract_js_imports() {
        let content = r#"
import express from 'express';
import { useState } from 'react';
import fs from 'fs';
const lodash = require('lodash');
import('@types/node');
"#;
        let imports = extract_js_imports(content, "test.js");

        let names: Vec<&str> = imports.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"express"));
        assert!(names.contains(&"react"));
        assert!(names.contains(&"lodash"));
        // fs is builtin
        assert!(!names.contains(&"fs"));
    }

    #[test]
    fn test_extract_npm_package_name() {
        assert_eq!(extract_npm_package_name("lodash"), "lodash");
        assert_eq!(extract_npm_package_name("lodash/get"), "lodash");
        assert_eq!(extract_npm_package_name("@types/node"), "@types/node");
        assert_eq!(extract_npm_package_name("@babel/core/lib"), "@babel/core");
    }

    #[test]
    fn test_extract_go_imports() {
        let content = r#"
package main

import (
    "fmt"
    "github.com/gin-gonic/gin"
    "github.com/spf13/cobra"
)

import "golang.org/x/net/context"
"#;
        let imports = extract_go_imports(content, "main.go");

        let names: Vec<&str> = imports.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"github.com/gin-gonic/gin"));
        assert!(names.contains(&"github.com/spf13/cobra"));
        assert!(names.contains(&"golang.org/x/net"));
        // fmt is stdlib
        assert!(!names.contains(&"fmt"));
    }

    #[test]
    fn test_extract_rust_imports() {
        let content = r#"
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use anyhow::Result;
"#;
        let imports = extract_rust_imports(content, "main.rs");

        let names: Vec<&str> = imports.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"serde"));
        assert!(names.contains(&"anyhow"));
        // std is builtin
        assert!(!names.contains(&"std"));
    }

    // Note: stdlib detection tests are in the stdlib module
}

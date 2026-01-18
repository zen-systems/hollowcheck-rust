//! Import extraction from source files.
//!
//! Extracts package imports using tree-sitter parsers and regex fallback.
//! Supports Python, JavaScript/TypeScript, Go, and Rust.

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
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

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
        // import foo, bar
        static ref IMPORT_RE: Regex = Regex::new(r"(?m)^import\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        // from foo import bar
        static ref FROM_IMPORT_RE: Regex = Regex::new(r"(?m)^from\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    }

    let mut imports = Vec::new();
    let mut seen = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // import foo
        if let Some(caps) = IMPORT_RE.captures(trimmed) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if !is_python_stdlib(&name) && seen.insert(name.clone()) {
                imports.push(ImportedDependency {
                    name,
                    registry: RegistryType::PyPI,
                    file: file.to_string(),
                    line: line_num + 1,
                });
            }
        }

        // from foo import bar
        if let Some(caps) = FROM_IMPORT_RE.captures(trimmed) {
            let name = caps.get(1).unwrap().as_str().to_string();
            if !is_python_stdlib(&name) && seen.insert(name.clone()) {
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

/// Check if a Python module is part of the standard library.
fn is_python_stdlib(name: &str) -> bool {
    const STDLIB: &[&str] = &[
        "abc", "aifc", "argparse", "array", "ast", "asynchat", "asyncio", "asyncore",
        "atexit", "audioop", "base64", "bdb", "binascii", "binhex", "bisect",
        "builtins", "bz2", "calendar", "cgi", "cgitb", "chunk", "cmath", "cmd",
        "code", "codecs", "codeop", "collections", "colorsys", "compileall",
        "concurrent", "configparser", "contextlib", "contextvars", "copy",
        "copyreg", "cProfile", "crypt", "csv", "ctypes", "curses", "dataclasses",
        "datetime", "dbm", "decimal", "difflib", "dis", "distutils", "doctest",
        "email", "encodings", "enum", "errno", "faulthandler", "fcntl", "filecmp",
        "fileinput", "fnmatch", "fractions", "ftplib", "functools", "gc", "getopt",
        "getpass", "gettext", "glob", "graphlib", "grp", "gzip", "hashlib", "heapq",
        "hmac", "html", "http", "idlelib", "imaplib", "imghdr", "imp", "importlib",
        "inspect", "io", "ipaddress", "itertools", "json", "keyword", "lib2to3",
        "linecache", "locale", "logging", "lzma", "mailbox", "mailcap", "marshal",
        "math", "mimetypes", "mmap", "modulefinder", "multiprocessing", "netrc",
        "nis", "nntplib", "numbers", "operator", "optparse", "os", "ossaudiodev",
        "pathlib", "pdb", "pickle", "pickletools", "pipes", "pkgutil", "platform",
        "plistlib", "poplib", "posix", "posixpath", "pprint", "profile", "pstats",
        "pty", "pwd", "py_compile", "pyclbr", "pydoc", "queue", "quopri", "random",
        "re", "readline", "reprlib", "resource", "rlcompleter", "runpy", "sched",
        "secrets", "select", "selectors", "shelve", "shlex", "shutil", "signal",
        "site", "smtpd", "smtplib", "sndhdr", "socket", "socketserver", "spwd",
        "sqlite3", "ssl", "stat", "statistics", "string", "stringprep", "struct",
        "subprocess", "sunau", "symtable", "sys", "sysconfig", "syslog", "tabnanny",
        "tarfile", "telnetlib", "tempfile", "termios", "test", "textwrap", "threading",
        "time", "timeit", "tkinter", "token", "tokenize", "trace", "traceback",
        "tracemalloc", "tty", "turtle", "turtledemo", "types", "typing", "unicodedata",
        "unittest", "urllib", "uu", "uuid", "venv", "warnings", "wave", "weakref",
        "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib", "xml", "xmlrpc",
        "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
        // Underscore prefixed private modules
        "_thread", "__future__",
    ];
    STDLIB.contains(&name)
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
                if !is_node_builtin(&pkg) && seen.insert(pkg.clone()) {
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
                if !is_node_builtin(&pkg) && seen.insert(pkg.clone()) {
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
        import_path.split('/').next().unwrap_or(import_path).to_string()
    }
}

/// Check if a module is a Node.js builtin.
fn is_node_builtin(name: &str) -> bool {
    const BUILTINS: &[&str] = &[
        "assert", "async_hooks", "buffer", "child_process", "cluster", "console",
        "constants", "crypto", "dgram", "diagnostics_channel", "dns", "domain",
        "events", "fs", "http", "http2", "https", "inspector", "module", "net",
        "os", "path", "perf_hooks", "process", "punycode", "querystring", "readline",
        "repl", "stream", "string_decoder", "sys", "timers", "tls", "trace_events",
        "tty", "url", "util", "v8", "vm", "wasi", "worker_threads", "zlib",
        // node: protocol prefix
        "node:assert", "node:buffer", "node:child_process", "node:cluster",
        "node:console", "node:crypto", "node:dgram", "node:dns", "node:events",
        "node:fs", "node:http", "node:http2", "node:https", "node:inspector",
        "node:module", "node:net", "node:os", "node:path", "node:perf_hooks",
        "node:process", "node:querystring", "node:readline", "node:repl",
        "node:stream", "node:string_decoder", "node:timers", "node:tls", "node:tty",
        "node:url", "node:util", "node:v8", "node:vm", "node:wasi",
        "node:worker_threads", "node:zlib",
    ];
    BUILTINS.contains(&name) || name.starts_with("node:")
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
            if is_external_go_import(import_path) {
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
                    if is_external_go_import(import_path) {
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

/// Check if a Go import is external (not stdlib).
fn is_external_go_import(path: &str) -> bool {
    // External imports have a domain (contain a dot in the first segment)
    // e.g., github.com/user/repo, golang.org/x/net
    path.contains('.') && !path.starts_with('.')
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
            if !is_rust_builtin(&name) && seen.insert(name.clone()) {
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
            if !is_rust_builtin(&name) && seen.insert(name.clone()) {
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

/// Check if a Rust crate is a builtin.
fn is_rust_builtin(name: &str) -> bool {
    const BUILTINS: &[&str] = &[
        // Standard library and core crates
        "std", "core", "alloc", "proc_macro", "test",
        // Special crates
        "self", "super", "crate",
    ];
    BUILTINS.contains(&name)
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

    #[test]
    fn test_is_python_stdlib() {
        assert!(is_python_stdlib("os"));
        assert!(is_python_stdlib("json"));
        assert!(is_python_stdlib("typing"));
        assert!(!is_python_stdlib("requests"));
        assert!(!is_python_stdlib("flask"));
    }

    #[test]
    fn test_is_node_builtin() {
        assert!(is_node_builtin("fs"));
        assert!(is_node_builtin("path"));
        assert!(is_node_builtin("node:fs"));
        assert!(!is_node_builtin("express"));
        assert!(!is_node_builtin("lodash"));
    }

    #[test]
    fn test_is_external_go_import() {
        assert!(is_external_go_import("github.com/user/repo"));
        assert!(is_external_go_import("golang.org/x/net"));
        assert!(!is_external_go_import("fmt"));
        assert!(!is_external_go_import("net/http"));
    }
}

//! Analysis context for caching parsed files and facts.
//!
//! The AnalysisContext provides:
//! - Caching of parsed files to avoid re-parsing
//! - Caching of extracted facts
//! - Cross-file symbol lookup
//! - Language-aware file grouping

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::analysis::{get_analyzer, Declaration, DeclarationKind, FileFacts};

/// Analysis context for a set of files.
///
/// This struct caches parsed ASTs and extracted facts to avoid re-parsing
/// and re-analyzing files multiple times during a scan.
pub struct AnalysisContext {
    /// Base directory for relative path resolution.
    base_dir: PathBuf,
    /// Cached file facts, keyed by absolute path.
    facts_cache: RwLock<HashMap<PathBuf, FileFacts>>,
    /// Mapping from relative path to absolute path.
    path_map: RwLock<HashMap<String, PathBuf>>,
}

impl AnalysisContext {
    /// Create a new analysis context.
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            facts_cache: RwLock::new(HashMap::new()),
            path_map: RwLock::new(HashMap::new()),
        }
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Analyze a file and cache the results.
    ///
    /// Returns cached facts if already analyzed.
    pub fn analyze_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<FileFacts> {
        let path = path.as_ref();
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        };

        // Check cache first
        {
            let cache = self.facts_cache.read().unwrap();
            if let Some(facts) = cache.get(&abs_path) {
                return Ok(facts.clone());
            }
        }

        // Determine language from extension
        let ext = abs_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let analyzer = get_analyzer(ext);
        if analyzer.is_none() {
            // Return empty facts for unsupported files
            let rel_path = abs_path
                .strip_prefix(&self.base_dir)
                .unwrap_or(&abs_path)
                .to_string_lossy()
                .to_string();

            return Ok(FileFacts::empty(&rel_path, "unknown"));
        }

        let analyzer = analyzer.unwrap();

        // Read and parse file
        let source = fs::read(&abs_path)?;
        let parsed = analyzer.parse(&abs_path, &source)?;
        let mut facts = analyzer.extract_facts(&parsed)?;

        // Store relative path in facts
        let rel_path = abs_path
            .strip_prefix(&self.base_dir)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .to_string();
        facts.path = rel_path.clone();

        // Cache the results
        {
            let mut cache = self.facts_cache.write().unwrap();
            cache.insert(abs_path.clone(), facts.clone());
        }

        {
            let mut path_map = self.path_map.write().unwrap();
            path_map.insert(rel_path, abs_path);
        }

        Ok(facts)
    }

    /// Analyze multiple files.
    ///
    /// Processes files sequentially to maintain deterministic ordering.
    /// For parallel processing, use `analyze_files_parallel`.
    pub fn analyze_files(&self, paths: &[PathBuf]) -> anyhow::Result<Vec<FileFacts>> {
        let mut all_facts = Vec::new();

        for path in paths {
            match self.analyze_file(path) {
                Ok(facts) => all_facts.push(facts),
                Err(e) => {
                    // Log but don't fail - some files may not be parseable
                    eprintln!("Warning: Failed to analyze file: {}", e);
                }
            }
        }

        // Sort by path for deterministic ordering
        all_facts.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(all_facts)
    }

    /// Analyze multiple files in parallel.
    ///
    /// Uses rayon for parallel processing. Results are sorted by path.
    pub fn analyze_files_parallel(&self, paths: &[PathBuf]) -> anyhow::Result<Vec<FileFacts>> {
        use rayon::prelude::*;

        let results: Vec<_> = paths
            .par_iter()
            .map(|p| self.analyze_file(p))
            .collect();

        // Collect results, logging errors but continuing
        let mut all_facts = Vec::new();
        for result in results {
            match result {
                Ok(facts) => all_facts.push(facts),
                Err(e) => {
                    // Log but don't fail - some files may not be parseable
                    eprintln!("Warning: Failed to analyze file: {}", e);
                }
            }
        }

        // Sort by path for deterministic ordering
        all_facts.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(all_facts)
    }

    /// Get cached facts for a file.
    ///
    /// Returns None if the file hasn't been analyzed yet.
    pub fn facts_for_file<P: AsRef<Path>>(&self, path: P) -> Option<FileFacts> {
        let path = path.as_ref();
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        };

        let cache = self.facts_cache.read().unwrap();
        cache.get(&abs_path).cloned()
    }

    /// Get all cached facts for a given language.
    pub fn facts_by_language(&self, language: &str) -> Vec<FileFacts> {
        let cache = self.facts_cache.read().unwrap();
        cache
            .values()
            .filter(|f| f.language == language)
            .cloned()
            .collect()
    }

    /// Find a symbol by name across all analyzed files.
    ///
    /// If `kind` is specified, only matches that kind.
    /// If `language` is specified, only searches files of that language.
    pub fn find_symbol(
        &self,
        name: &str,
        kind: Option<DeclarationKind>,
        language: Option<&str>,
    ) -> Vec<SymbolMatch> {
        let cache = self.facts_cache.read().unwrap();
        let mut matches = Vec::new();

        for facts in cache.values() {
            if let Some(lang) = language {
                if facts.language != lang {
                    continue;
                }
            }

            for decl in &facts.declarations {
                if decl.name != name {
                    continue;
                }

                if let Some(k) = kind {
                    if decl.kind != k {
                        continue;
                    }
                }

                matches.push(SymbolMatch {
                    declaration: decl.clone(),
                    file: facts.path.clone(),
                    language: facts.language.clone(),
                });
            }
        }

        // Sort for deterministic output
        matches.sort_by(|a, b| (&a.file, a.declaration.span.start_byte)
            .cmp(&(&b.file, b.declaration.span.start_byte)));

        matches
    }

    /// Get all declarations of a specific kind across all analyzed files.
    pub fn declarations_by_kind(&self, kind: DeclarationKind) -> Vec<SymbolMatch> {
        let cache = self.facts_cache.read().unwrap();
        let mut matches = Vec::new();

        for facts in cache.values() {
            for decl in &facts.declarations {
                if decl.kind == kind {
                    matches.push(SymbolMatch {
                        declaration: decl.clone(),
                        file: facts.path.clone(),
                        language: facts.language.clone(),
                    });
                }
            }
        }

        // Sort for deterministic output
        matches.sort_by(|a, b| (&a.file, a.declaration.span.start_byte)
            .cmp(&(&b.file, b.declaration.span.start_byte)));

        matches
    }

    /// Get all analyzed file paths.
    pub fn analyzed_files(&self) -> Vec<String> {
        let cache = self.facts_cache.read().unwrap();
        let mut files: Vec<_> = cache.values().map(|f| f.path.clone()).collect();
        files.sort();
        files
    }

    /// Clear the cache.
    pub fn clear_cache(&self) {
        let mut cache = self.facts_cache.write().unwrap();
        cache.clear();

        let mut path_map = self.path_map.write().unwrap();
        path_map.clear();
    }
}

/// A symbol match result.
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    /// The declaration that matched.
    pub declaration: Declaration,
    /// The file containing the declaration.
    pub file: String,
    /// The language of the file.
    pub language: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_analyze_go_file() {
        // Initialize analyzers
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        fs::write(
            &file_path,
            r#"
package main

func main() {
    println("hello")
}

func helper() int {
    return 42
}
"#,
        )
        .unwrap();

        let ctx = AnalysisContext::new(temp.path());
        let facts = ctx.analyze_file(&file_path).unwrap();

        assert_eq!(facts.language, "go");
        assert_eq!(facts.package, Some("main".to_string()));
        assert_eq!(facts.declarations.len(), 2);
    }

    #[test]
    fn test_caching() {
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("main.go");
        fs::write(&file_path, "package main\nfunc main() {}").unwrap();

        let ctx = AnalysisContext::new(temp.path());

        // First analysis
        let facts1 = ctx.analyze_file(&file_path).unwrap();

        // Second analysis should return cached result
        let facts2 = ctx.analyze_file(&file_path).unwrap();

        assert_eq!(facts1.path, facts2.path);
        assert_eq!(facts1.declarations.len(), facts2.declarations.len());
    }

    #[test]
    fn test_find_symbol() {
        crate::analysis::register_analyzers();

        let temp = TempDir::new().unwrap();

        let file1 = temp.path().join("a.go");
        fs::write(&file1, "package main\nfunc helper() {}").unwrap();

        let file2 = temp.path().join("b.go");
        fs::write(&file2, "package main\nfunc helper() {}\nfunc other() {}").unwrap();

        let ctx = AnalysisContext::new(temp.path());
        ctx.analyze_file(&file1).unwrap();
        ctx.analyze_file(&file2).unwrap();

        // Find 'helper' in all files
        let matches = ctx.find_symbol("helper", None, None);
        assert_eq!(matches.len(), 2);

        // Find 'other' (only in b.go)
        let matches = ctx.find_symbol("other", None, None);
        assert_eq!(matches.len(), 1);

        // Find non-existent symbol
        let matches = ctx.find_symbol("nonexistent", None, None);
        assert_eq!(matches.len(), 0);
    }
}

//! Detection of missing required files.

use crate::contract::RequiredFile;
use std::path::Path;

use super::{DetectionResult, Severity, Violation, ViolationRule};

/// Check that all required files exist.
pub fn detect_missing_files<P: AsRef<Path>>(
    base_dir: P,
    files: &[RequiredFile],
) -> anyhow::Result<DetectionResult> {
    let mut result = DetectionResult::new();
    let base = base_dir.as_ref();

    for f in files {
        if !f.required {
            continue;
        }

        let full_path = base.join(&f.path);
        match std::fs::metadata(&full_path) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    result.add_violation(Violation {
                        rule: ViolationRule::MissingFile,
                        message: format!("required file {:?} is a directory, not a file", f.path),
                        file: f.path.clone(),
                        line: 0,
                        severity: Severity::Error,
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                result.add_violation(Violation {
                    rule: ViolationRule::MissingFile,
                    message: format!("required file {:?} does not exist", f.path),
                    file: f.path.clone(),
                    line: 0,
                    severity: Severity::Error,
                });
            }
            Err(e) => {
                return Err(anyhow::anyhow!("checking file {}: {}", f.path, e));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_missing_files() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("exists.txt"), "content").unwrap();

        let files = vec![
            RequiredFile {
                path: "exists.txt".to_string(),
                required: true,
            },
            RequiredFile {
                path: "missing.txt".to_string(),
                required: true,
            },
            RequiredFile {
                path: "optional.txt".to_string(),
                required: false,
            },
        ];

        let result = detect_missing_files(temp.path(), &files).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].rule, ViolationRule::MissingFile);
        assert!(result.violations[0].message.contains("missing.txt"));
    }

    #[test]
    fn test_detect_directory_as_file() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir(temp.path().join("subdir")).unwrap();

        let files = vec![RequiredFile {
            path: "subdir".to_string(),
            required: true,
        }];

        let result = detect_missing_files(temp.path(), &files).unwrap();
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].message.contains("is a directory"));
    }
}

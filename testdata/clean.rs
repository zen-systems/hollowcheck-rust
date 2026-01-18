// Test fixtures for hollowcheck - clean implementation patterns.

use std::error::Error;
use std::fmt;

/// Represents application configuration.
pub struct Config {
    pub max_retries: i32,
    pub timeout: i32,
    pub debug: bool,
}

/// Maximum number of allowed connections.
pub const MAX_CONNECTIONS: usize = 100;

/// Error for invalid input.
#[derive(Debug)]
pub struct InvalidInputError;

impl fmt::Display for InvalidInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid input")
    }
}

impl Error for InvalidInputError {}

impl Config {
    /// Checks if the configuration is valid.
    /// This function has real logic with multiple decision points.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_retries < 0 {
            return Err("max retries cannot be negative".to_string());
        }
        if self.max_retries > 10 {
            return Err("max retries cannot exceed 10".to_string());
        }
        if self.timeout <= 0 {
            return Err("timeout must be positive".to_string());
        }
        if self.timeout > 300 {
            return Err("timeout cannot exceed 300 seconds".to_string());
        }
        Ok(())
    }
}

/// Processes a list of items with actual logic.
/// This demonstrates a function with reasonable cyclomatic complexity.
pub fn process_items(items: &[String], config: &Config) -> Result<Vec<String>, Box<dyn Error>> {
    if items.is_empty() {
        return Err(Box::new(InvalidInputError));
    }

    let mut result = Vec::with_capacity(items.len());
    let retries = 0;

    for item in items {
        if item.is_empty() {
            continue;
        }

        let mut processed = item.trim().to_string();
        if processed.is_empty() {
            continue;
        }

        if processed.starts_with('#') {
            // Skip comments
            continue;
        }

        if config.debug && processed.len() > 100 {
            processed = processed[..100].to_string();
        }

        result.push(processed.to_lowercase());

        if result.len() >= MAX_CONNECTIONS {
            break;
        }
    }

    if result.is_empty() && retries < config.max_retries {
        return Err("no valid items found after processing".into());
    }

    Ok(result)
}

/// Computes a score based on multiple factors.
pub fn calculate_score(values: &[i32], threshold: i32) -> i32 {
    if values.is_empty() {
        return 0;
    }

    let mut sum = 0;
    let mut count = 0;

    for &v in values {
        if v < 0 {
            continue;
        }
        if v > threshold {
            sum += threshold;
        } else {
            sum += v;
        }
        count += 1;
    }

    if count == 0 {
        return 0;
    }

    let avg = sum / count;
    if avg > 100 {
        return 100;
    }
    avg
}

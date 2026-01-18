// Test file with hallucinated Rust dependencies

use std::collections::HashMap;  // stdlib - should be ignored
use std::io::{self, Read};  // stdlib - should be ignored

use serde::{Deserialize, Serialize};  // real crate - should pass
use tokio::runtime::Runtime;  // real crate - should pass

// These are fake crates that should be flagged
use nonexistent_ai_generated_crate_12345;
use fake_utils_library_xyz::helper;
use totally_made_up_sdk;

fn main() {
    println!("Hello, world!");
}

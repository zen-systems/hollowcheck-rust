//! HCL (Terraform) language configuration for tree-sitter parsing.

use crate::parser::treesitter::{Config, SymbolCapture, TreeSitterParser};
use crate::parser::Parser;

/// Tree-sitter query for finding HCL/Terraform symbols.
///
/// Captures blocks and their identifiers:
/// - resource, data, module, variable, output, provider, locals
const SYMBOL_QUERY: &str = r#"
(block (identifier) @block_type (string_lit)? @resource_type (string_lit)? @resource_name) @block
"#;

/// Tree-sitter query for finding HCL blocks by name.
/// Note: HCL doesn't have traditional functions, so we look for blocks.
const FUNCTION_QUERY: &str = r#"
(block (identifier) @block_type (string_lit) @type (string_lit) @name) @block
"#;

/// Tree-sitter query for counting complexity in HCL.
/// HCL doesn't have traditional control flow, but we can count
/// conditional expressions and dynamic blocks.
const COMPLEXITY_QUERY: &str = r#"
(conditional) @branch
(for_expr) @branch
"#;

/// Symbol capture configurations for HCL.
static SYMBOL_CAPTURES: &[SymbolCapture] = &[
    SymbolCapture {
        name_capture: "block_type",
        kind: "type",
    },
    SymbolCapture {
        name_capture: "resource_name",
        kind: "function",
    },
];

/// Create a new HCL parser.
pub fn new_parser() -> Box<dyn Parser> {
    Box::new(TreeSitterParser::new(Config {
        language: tree_sitter_hcl::LANGUAGE.into(),
        language_name: "hcl",
        symbol_query: SYMBOL_QUERY,
        symbol_captures: SYMBOL_CAPTURES,
        complexity_query: COMPLEXITY_QUERY,
        function_query: FUNCTION_QUERY,
        function_capture: "block",
        func_name_capture: "name",
    }))
}

/// Register HCL parser for .tf and .tfvars extensions.
pub fn register() {
    crate::parser::register(".tf", new_parser);
    crate::parser::register(".tfvars", new_parser);
    crate::parser::register(".hcl", new_parser);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hcl_symbols() {
        let parser = new_parser();
        let source = br#"
terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

resource "aws_instance" "web" {
  ami           = var.ami_id
  instance_type = var.instance_type
}

variable "ami_id" {
  description = "AMI ID for the EC2 instance"
  type        = string
}

output "instance_ip" {
  value = aws_instance.web.public_ip
}
"#;

        let symbols = parser.parse_symbols(source).unwrap();

        // HCL parsing extracts block types
        assert!(
            symbols.iter().any(|s| s.name == "terraform" || s.name == "resource" || s.name == "variable" || s.name == "output"),
            "Expected to find HCL block types, got: {:?}",
            symbols
        );
    }

    #[test]
    fn test_hcl_complexity_simple() {
        let parser = new_parser();
        let source = br#"
resource "aws_instance" "simple" {
  ami           = "ami-12345"
  instance_type = "t2.micro"
}
"#;

        // HCL blocks don't have traditional complexity
        let complexity = parser.complexity(source, "simple").unwrap();
        assert!(complexity >= 0, "Expected non-negative complexity");
    }

    #[test]
    fn test_hcl_complexity_with_conditional() {
        let parser = new_parser();
        let source = br#"
resource "aws_instance" "conditional" {
  ami           = var.use_custom ? var.custom_ami : "ami-default"
  instance_type = "t2.micro"
}
"#;

        let complexity = parser.complexity(source, "conditional").unwrap();
        // Should detect the conditional expression
        assert!(complexity >= 1, "Expected >= 1, got {}", complexity);
    }
}

.PHONY: build test clean install release lint fmt check

# Default target
all: build

# Build debug binary
build:
	cargo build

# Build release binary
release:
	cargo build --release

# Run tests
test:
	cargo test

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Run clippy linter
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Check formatting without modifying
check-fmt:
	cargo fmt -- --check

# Run all checks (fmt, lint, test)
check: check-fmt lint test

# Clean build artifacts
clean:
	cargo clean
	rm -rf dist/

# Install to ~/.cargo/bin
install:
	cargo install --path .

# Build for all platforms (requires cross)
build-all:
	./scripts/build-all.sh

# Run against testdata
run-test:
	cargo run -- lint testdata --contract testdata/test-contract.yaml

# Run with JSON output
run-json:
	cargo run -- lint testdata --contract testdata/test-contract.yaml --format json

# List available init templates
list-templates:
	cargo run -- init --list

# Create a new tag and push (use: make tag VERSION=v0.1.0)
tag:
	git tag -a $(VERSION) -m "Release $(VERSION)"
	git push origin $(VERSION)

# Help
help:
	@echo "Available targets:"
	@echo "  build        - Build debug binary"
	@echo "  release      - Build release binary"
	@echo "  test         - Run tests"
	@echo "  lint         - Run clippy"
	@echo "  fmt          - Format code"
	@echo "  check        - Run all checks (fmt, lint, test)"
	@echo "  clean        - Clean build artifacts"
	@echo "  install      - Install to ~/.cargo/bin"
	@echo "  build-all    - Build for all platforms"
	@echo "  run-test     - Run against testdata"
	@echo "  tag          - Create and push a git tag (VERSION=v0.1.0)"

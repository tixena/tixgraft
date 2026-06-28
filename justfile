# TixGraft project justfile
# Run `just --list` to see all available commands

# Default recipe - show help
default:
    @just --list

# Run code formatting
fmt:
    @echo "Formatting code..."
    cargo fmt --all

# Check code formatting without making changes
fmt-check:
    @echo "Checking code formatting..."
    cargo fmt --all -- --check

# Run linting with clippy (lint config lives in Cargo.toml [lints.clippy])
lint:
    @echo "Running clippy lints..."
    cargo clippy --all-targets --all-features

# Run all tests
test:
    @echo "Running tests..."
    cargo test --all-features

# Run tests with output
test-verbose:
    @echo "Running tests (verbose)..."
    cargo test --all-features -- --nocapture

# Run specific test
test-filter FILTER:
    @echo "Running tests matching: {{FILTER}}"
    cargo test --all-features {{FILTER}}

# Build in debug mode
build:
    @echo "Building debug binary..."
    cargo build

# Build optimized release binary
build-release:
    @echo "Building release binary..."
    cargo build --release
    @echo "Binary available at: target/release/tixgraft"
    @just _show-binary-info

# Full pipeline (standardized `all` entry point across tixena repos).
all: fmt-check lint test build
    @echo "All checks completed successfully!"

# Full build pipeline - format, lint, test, and build
ci: fmt-check lint test build-release
    @echo "CI pipeline completed successfully!"

# Quick development check - format, lint, and test
dev: fmt lint test
    @echo "Development checks completed!"

# Clean build artifacts
clean:
    @echo "Cleaning build artifacts..."
    cargo clean

# Install from local source to ~/.cargo/bin
install:
    @echo "Installing tixgraft from local source..."
    cargo install --path . --force
    @echo "TixGraft installed successfully!"
    @echo "Run 'tixgraft --version' to verify installation"

# Install from crates.io
install-release:
    @echo "Installing tixgraft from crates.io..."
    cargo install tixgraft
    @echo "TixGraft installed successfully!"

# Run the binary with example arguments
run *ARGS:
    @echo "Running tixgraft with args: {{ARGS}}"
    cargo run -- {{ARGS}}

# Generate documentation
docs:
    @echo "Generating documentation..."
    cargo doc --all-features --no-deps --open

# Check for security vulnerabilities
audit:
    @echo "Checking for security vulnerabilities..."
    cargo audit

# Update dependencies
update:
    @echo "Updating dependencies..."
    cargo update

# Show outdated dependencies
outdated:
    @echo "Checking for outdated dependencies..."
    cargo outdated

# Create a new release (for maintainers)
release VERSION:
    @echo "Preparing release {{VERSION}}..."
    @echo "Updating Cargo.toml version..."
    sed -i '' 's/version = ".*"/version = "{{VERSION}}"/' Cargo.toml
    @echo "Please update CHANGELOG.md with version {{VERSION}}"
    @echo "Run 'just ci' to verify everything works"
    @echo "Then run 'git tag v{{VERSION}}' to create release tag"

# Check binary size and dependencies
bloat:
    @echo "Analyzing binary size..."
    cargo bloat --release

# Verify all examples compile and validate
validate-examples:
    @echo "Validating example configurations..."
    @find docs/examples -name "*.yaml" -exec sh -c 'echo "Validating: $$1"; cargo run -- --config "$$1" --dry-run' sh {} \;
    @echo "All examples validated successfully!"

# Generate code coverage report (requires cargo-llvm-cov)
test-coverage:
    @echo "Generating code coverage report..."
    cargo llvm-cov --workspace --all-features

# Generate code coverage HTML report (requires cargo-llvm-cov)
test-coverage-html:
    @echo "Generating code coverage HTML report..."
    cargo llvm-cov --workspace --all-features --html
    @echo "Coverage report generated in target/llvm-cov/html/"

# Show binary information (private recipe)
_show-binary-info:
    #!/usr/bin/env bash
    if [[ -f "target/release/tixgraft" ]]; then
        echo "Binary info:"
        if command -v file >/dev/null 2>&1; then
            file target/release/tixgraft
        fi
        echo "Binary size:"
        if command -v ls >/dev/null 2>&1; then
            ls -lh target/release/tixgraft | awk '{print $5}'
        fi
    fi

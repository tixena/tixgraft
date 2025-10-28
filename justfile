# TixGraft project justfile
# Run `just --list` to see all available commands

# Default recipe - show help
default:
    @just --list

# Install dependencies and check environment
setup:
    @echo "🔧 Setting up tixgraft development environment..."
    @echo "📋 Checking Rust installation..."
    cargo --version
    @echo "📋 Checking Git installation..."
    git --version
    @echo "✅ Environment setup complete!"

# Run code formatting
fmt:
    @echo "📝 Formatting code..."
    cargo fmt --all

# Check code formatting without making changes
fmt-check:
    @echo "📝 Checking code formatting..."
    cargo fmt --all -- --check

# Run linting with clippy
lint:
    @echo "🔍 Running clippy lints..."
    cargo clippy --all-targets --all-features -- -D warnings

clippy-lint:
    set -e
    cargo fix --allow-dirty
    cargo clippy --all --fix --allow-dirty -- -W clippy::all -W clippy::pedantic -W clippy::restriction -W clippy::nursery -D warnings
    cargo fmt 

# Run all tests
test:
    @echo "🧪 Running tests..."
    cargo test --all-features

# Run tests with output
test-verbose:
    @echo "🧪 Running tests (verbose)..."
    cargo test --all-features -- --nocapture

# Run specific test
test-filter FILTER:
    @echo "🧪 Running tests matching: {{FILTER}}"
    cargo test --all-features {{FILTER}}

# Check that the code compiles
check:
    @echo "🔍 Checking compilation..."
    cargo check --all-targets --all-features

# Build in debug mode
build:
    @echo "🏗️ Building debug binary..."
    cargo build

# Build optimized release binary
build-release:
    @echo "🚀 Building release binary..."
    cargo build --release
    @echo "📦 Binary available at: target/release/tixgraft"
    @just _show-binary-info

# Full build pipeline - format, lint, test, and build
ci: fmt-check lint test build-release
    @echo "✅ CI pipeline completed successfully!"

# Quick development check - format, lint, and test
dev: fmt lint test
    @echo "✅ Development checks completed!"

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean

# Install from local source to ~/.cargo/bin
install:
    @echo "📦 Installing tixgraft from local source..."
    cargo install --path . --force
    @echo "✅ TixGraft installed successfully!"
    @echo "💡 Run 'tixgraft --version' to verify installation"

# Install from crates.io
install-release:
    @echo "📦 Installing tixgraft from crates.io..."
    cargo install tixgraft
    @echo "✅ TixGraft installed successfully!"

# Install from GitHub repository
install-git:
    @echo "📦 Installing tixgraft from GitHub..."
    cargo install --git https://github.com/tixena/tixgraft
    @echo "✅ TixGraft installed successfully!"

# Run the binary with example arguments
run *ARGS:
    @echo "🚀 Running tixgraft with args: {{ARGS}}"
    cargo run -- {{ARGS}}

# Run the release binary with example arguments
run-release *ARGS:
    @echo "🚀 Running release tixgraft with args: {{ARGS}}"
    ./target/release/tixgraft {{ARGS}}

# Run with dry-run flag for testing
dry-run:
    @echo "🔍 Running tixgraft dry-run with example config..."
    cargo run -- --dry-run --config docs/examples/basic-usage.yaml

# Generate documentation
docs:
    @echo "📚 Generating documentation..."
    cargo doc --all-features --no-deps --open

# Check for security vulnerabilities
audit:
    @echo "🛡️ Checking for security vulnerabilities..."
    cargo audit

# Update dependencies
update:
    @echo "📦 Updating dependencies..."
    cargo update

# Show outdated dependencies
outdated:
    @echo "📊 Checking for outdated dependencies..."
    cargo outdated

# Create a new release (for maintainers)
release VERSION:
    @echo "🎉 Preparing release {{VERSION}}..."
    @echo "📝 Updating Cargo.toml version..."
    sed -i '' 's/version = ".*"/version = "{{VERSION}}"/' Cargo.toml
    @echo "📝 Please update CHANGELOG.md with version {{VERSION}}"
    @echo "💡 Run 'just ci' to verify everything works"
    @echo "💡 Then run 'git tag v{{VERSION}}' to create release tag"

# Benchmark performance (if benchmarks exist)
bench:
    @echo "⏱️ Running benchmarks..."
    cargo bench

# Watch for changes and run tests
watch:
    @echo "👀 Watching for changes and running tests..."
    cargo watch -x test

# Watch for changes and run specific command
watch-run COMMAND:
    @echo "👀 Watching for changes and running: {{COMMAND}}"
    cargo watch -x "{{COMMAND}}"

# Profile the application (requires cargo-profiler)
profile:
    @echo "📊 Profiling application..."
    cargo build --release
    @echo "💡 Run profiling tools on: target/release/tixgraft"

# Check binary size and dependencies
bloat:
    @echo "📏 Analyzing binary size..."
    cargo bloat --release

# Verify all examples compile and validate
validate-examples:
    @echo "✅ Validating example configurations..."
    @find docs/examples -name "*.yaml" -exec sh -c 'echo "📋 Validating: $$1"; cargo run -- --config "$$1" --dry-run' sh {} \;
    @echo "✅ All examples validated successfully!"

# Check for TODO items and fixme in code
todos:
    @echo "📝 Searching for TODO and FIXME items..."
    @rg -i "todo|fixme|hack|bug" --type rust src/ || echo "No TODOs found!"

# Generate code coverage report
coverage:
    @echo "📊 Generating code coverage report..."
    cargo tarpaulin --all-features --out Html --output-dir coverage
    @echo "📋 Coverage report generated in coverage/"

# Setup git hooks
setup-hooks:
    @echo "🪝 Setting up git hooks..."
    @mkdir -p .git/hooks
    @echo '#!/bin/sh\njust fmt lint test' > .git/hooks/pre-commit
    @chmod +x .git/hooks/pre-commit
    @echo "✅ Pre-commit hook installed!"

# Remove git hooks
remove-hooks:
    @echo "🗑️ Removing git hooks..."
    @rm -f .git/hooks/pre-commit
    @echo "✅ Git hooks removed!"

# Show project statistics
stats:
    @echo "📊 Project Statistics"
    @echo "===================="
    @echo "📁 Lines of Rust code:"
    @find src -name "*.rs" -exec wc -l {} + | tail -1 | awk '{print $1}'
    @echo "📁 Total files:"
    @find src -name "*.rs" | wc -l
    @echo "📦 Dependencies:"
    @cargo tree --depth 1 | wc -l
    @echo "🧪 Test files:"
    @find . -name "*test*.rs" -o -path "*/tests/*" | wc -l

# Show binary information (private recipe)
_show-binary-info:
    #!/usr/bin/env bash
    if [[ -f "target/release/tixgraft" ]]; then
        echo "📋 Binary info:"
        if command -v file >/dev/null 2>&1; then
            file target/release/tixgraft
        fi
        echo "📏 Binary size:"
        if command -v ls >/dev/null 2>&1; then
            ls -lh target/release/tixgraft | awk '{print $5}'
        fi
    fi

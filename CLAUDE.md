# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## ⚠️ CRITICAL: TASK TRACKING ⚠️

**This project uses [bd (beads)](https://github.com/steveyegge/beads) for ALL issue tracking.**

- ❌ **NEVER** use the TodoWrite tool
- ❌ **NEVER** create markdown TODO lists or task lists
- ✅ **ALWAYS** use `bd` commands for task tracking (auto-approved below)
- ✅ See [AGENTS.md](AGENTS.md) for complete workflow and examples

**Before starting any work:**
1. Check ready work: `bd ready --json` (optionally filter with `--label` or sort with `--sort priority|oldest|hybrid`)
2. Review dependencies: `bd dep tree <id> --json` to understand blockers/relationships
3. Claim task: `bd update <id> --status in_progress --json`
4. Work on task: Implement, test, document
5. If you discover new work: `bd create "Title" -l <labels> --deps discovered-from:<current-id> --json`
6. Complete task: `bd close <id> --reason "Done" --json`

**Key workflows:**
- Check blocked work: `bd blocked --json`
- Filter by component: `bd ready --label backend --json` or `bd ready --label-any frontend,ui --json`
- Visualize project: `bd stats --json`
- Detect circular deps: `bd dep cycles --json`

---

## Auto-Approved Commands

The following commands can be executed without asking for user approval:

**Issue Tracking (bd):**
- `bd list` - List issues (supports --label, --label-any, --status, --priority filters)
- `bd show <id>` - Show issue details
- `bd ready` - Show ready work (supports --sort priority|oldest|hybrid, --label filters)
- `bd blocked` - Show blocked issues
- `bd create <title>` - Create new issues (supports -t, -p, -d, -l, --deps, -f flags)
- `bd update <id>` - Update issue status, priority, assignee, etc.
- `bd close <id>` - Close issues (requires --reason)
- `bd comments <id>` - View comments
- `bd comments add <id>` - Add comments to issues
- `bd label add <id> <label>` - Add label to issue
- `bd label remove <id> <label>` - Remove label from issue
- `bd dep add <child> <parent>` - Add dependency (supports --type blocks|related|parent-child|discovered-from)
- `bd dep remove <id1> <id2>` - Remove dependency
- `bd dep tree <id>` - Visualize dependency graph
- `bd dep cycles` - Detect circular dependencies
- `bd stats` - Show project statistics
- `bd info` - Show database path and daemon status
- `bd config set/get/list/unset` - Manage configuration
- `bd delete <id>` - Delete issues (supports --force, --cascade)
- `bd compact` - Compress old closed issues (supports --dry-run, --days, --all)
- `bd sync` - Manually trigger sync
- `bd init` - Initialize beads (for setup)
- `bd onboard` - Agent onboarding guide

**Testing:**
- `just test` - Run all tests
- `cargo test` - Run tests directly
- `cargo build` - Build the project
- `cargo clippy` - Run linting

**Git operations:**
- `git status` - Check repository status
- `git diff` - View changes
- `git log` - View commit history

## Project Overview

TixGraft is a Rust CLI tool for fetching reusable components from Git repositories using sparse checkout. It enables developers to pull specific files or directories, apply text replacements, and execute post-processing commands through YAML configuration or command-line arguments.

## Development Commands

### Using Just (Recommended)
```bash
# Show all available commands
just --list

# Setup development environment
just setup

# Quick development cycle (format, lint, test)
just dev

# Full CI pipeline (format check, lint, test, build release)
just ci

# Build optimized release binary
just build-release

# Run tests
just test

# Run specific test
just test-filter <FILTER>

# Run with verbose output
just test-verbose

# Format code
just fmt

# Run linting
just lint

# Install from local source
just install
```

### Using Cargo Directly
```bash
# Build debug binary
cargo build

# Build release binary
cargo build --release

# Run tests
cargo test --all-features

# Run clippy linting
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt --all

# Run the CLI
cargo run -- <args>
```

## Architecture

### Module Structure

The codebase follows a modular architecture with clear separation of concerns:

- **`cli/`** - Command-line interface using clap
  - `args.rs` - CLI argument parsing and structures
  - `commands.rs` - Command handling logic

- **`config/`** - Configuration management
  - `yaml.rs` - YAML parsing
  - `schema.rs` - JSON schema definitions
  - `validation.rs` - Configuration validation logic
  - Handles merging of YAML config with CLI arguments (CLI takes precedence)

- **`git/`** - Git operations
  - `repository.rs` - Repository URL handling and normalization
  - `sparse_checkout.rs` - Git sparse checkout implementation using git2

- **`operations/`** - Core functionality
  - `pull.rs` - Orchestrates the complete pull operation workflow
  - `copy.rs` - File copying logic with directory/file type handling
  - `replace.rs` - Text replacement in files (static values or environment variables)
  - `commands.rs` - Command execution in target directory context
  - `to_command_line.rs` - Convert config to CLI arguments (shell/JSON output)

- **`system/`** - System abstraction layer (critical for testing)
  - `mod.rs` - System trait, TempDirHandle trait, and WalkEntry
  - `real.rs` - Production implementation using std::fs and std::env
  - `mock.rs` - In-memory implementation for fast, isolated unit tests

- **`error/`** - Error handling
  - `types.rs` - Custom error types with specific exit codes (1-5)

- **`utils/`** - Utility functions
  - `path.rs` - Path validation and security (prevents directory traversal)
  - `fs.rs` - File system helpers

### Core Flow

1. **CLI Parsing** (`main.rs` → `cli::Args::parse()`)
   - Parses command-line arguments using clap

2. **Configuration Loading** (`PullOperation::new()`)
   - Loads YAML config file if exists (default: `./tixgraft.yaml`)
   - Merges CLI arguments into config (CLI takes precedence)
   - Validates configuration against JSON schema

3. **Pull Execution** (`PullOperation::execute()`)
   For each pull operation (all filesystem operations go through System abstraction):
   - **Sparse Checkout**: Uses git2 to fetch only required paths from repository
   - **Source Verification**: Confirms source path exists in checked-out repository
   - **File Copy**: Copies files/directories to target location via `system.copy()` and `system.walk_dir()`
   - **Text Replacement**: Applies placeholder replacements using `system.read/write()`
   - **Command Execution**: Runs post-processing commands in target directory

### Key Design Patterns

- **System Abstraction**: All filesystem and environment operations go through the `System` trait
  - **RealSystem**: Production implementation that delegates to `std::fs` and `std::env` (zero-cost abstraction)
  - **MockSystem**: In-memory implementation for testing (no disk I/O, perfect isolation)
  - Key capabilities:
    - `create_temp_dir()` - System-agnostic temporary directories with automatic cleanup
    - `walk_dir()` - Recursive directory traversal (respects .gitignore in RealSystem, in-memory in MockSystem)
    - `read/write/copy/exists/is_dir/is_file` - All filesystem operations abstracted
  - **Critical**: Unit tests MUST use MockSystem, never RealSystem (see Testing Guidelines below)

- **Error Handling**: Custom `GraftError` enum with specific exit codes for different error types (configuration=1, source=2, command=3, git=4, filesystem=5)

- **Configuration Hierarchy**: Global settings (repository, tag) can be overridden per-pull operation, and CLI arguments override everything

- **Sparse Checkout Strategy**: Uses Git sparse checkout to efficiently fetch only needed files/directories, avoiding full repository clones

- **Security**: Path validation prevents directory traversal attacks; binary files are skipped during text replacement

## Configuration

### Repository URL Formats
- Short: `my_organization/repo` → expands to `https://github.com/my_organization/repo.git`
- HTTPS: `https://github.com/my_organization/repo.git`
- SSH: `git@github.com:my_organization/repo.git`

### YAML Structure
```yaml
repository: "my_organization/scaffolds"  # Optional global repo
tag: "main"                    # Optional global ref

pulls:                         # Required, minimum 1
  - source: "path/in/repo"     # Required
    target: "./local/path"     # Required
    type: "directory"          # Optional: "file" or "directory"
    repository: "override/repo" # Optional: per-pull override
    tag: "v1.0.0"              # Optional: per-pull override
    reset: true                # Optional: rm -rf target before copy
    commands:                  # Optional: post-copy commands
      - "npm install"
    replacements:              # Optional: text replacements
      - source: "{{PLACEHOLDER}}"
        target: "value"        # Static value
      - source: "{{VAR}}"
        valueFromEnv: "ENV_VAR" # From environment
```

## Testing Guidelines

### Test Structure

Tests are organized in `tests/`:
- `*_unit_tests.rs` - Unit tests (MUST use MockSystem)
- `*_tests.rs` - Integration tests (may use RealSystem when necessary)

### Unit Tests (Critical Rules)

**Unit tests MUST follow these rules:**

✅ **DO:**
- Use `MockSystem::new()` for all filesystem operations
- Use `system.create_temp_dir()` for temporary directories
- Use `.with_file()` and `.with_dir()` to set up test data
- Use mock paths like `/test/...` for clarity
- Test business logic in isolation without I/O

❌ **DON'T:**
- Never instantiate `RealSystem` in unit tests
- Never use `tempfile::TempDir` directly (use `system.create_temp_dir()`)
- Never use `std::fs` operations directly
- Never perform actual disk I/O in unit tests

**Example Unit Test:**
```rust
use tixgraft::system::{MockSystem, System};

#[test]
fn my_feature() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/input.txt", b"test data");

    let temp_dir = system.create_temp_dir().unwrap();
    // temp_dir.path() is now an in-memory directory like /tmp/mock_0

    // Test your feature...
    assert!(system.exists(temp_dir.path()));
}
```

### Integration Tests

**Use RealSystem only when testing:**
- Actual git operations (`sparse_checkout_tests.rs`)
- Shell command execution (`commands_unit_tests.rs`, `post_commands_tests.rs`)
- End-to-end CLI behavior (`cli_tests.rs`, `local_tests.rs`)

**Still prefer System abstraction:**
```rust
use tixgraft::system::{RealSystem, System};

#[test]
fn integration() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    // temp_dir automatically cleans up on drop

    // Test with real filesystem...
}
```

### Test Files

**Unit Tests (MockSystem only):**
- `copy_unit_tests.rs` - File and directory copying
- `discovery_unit_tests.rs` - Graft file discovery
- `fs_unit_tests.rs` - Filesystem utilities
- `gitignore_tests.rs` - Directory traversal (simplified for MockSystem)
- `replace_unit_tests.rs` - Text replacement
- `yaml_unit_tests.rs` - YAML config loading
- `temp_dir_tests.rs` - TempDir abstraction

**Integration Tests (RealSystem when needed):**
- `cli_tests.rs` - CLI argument parsing and execution
- `commands_unit_tests.rs` - Command execution (requires real shell)
- `config_tests.rs` - Configuration loading and validation
- `local_tests.rs` - Local filesystem operations
- `post_commands_tests.rs` - Post-command execution
- `sparse_checkout_tests.rs` - Git operations (requires real git)
- `to_command_line_tests.rs` - Config to CLI conversion
- `to_config_tests.rs` - CLI to config conversion

### Benefits of System Abstraction

- **Fast**: MockSystem tests run ~100x faster (no disk I/O)
- **Isolated**: No temp directory cleanup issues or race conditions
- **Deterministic**: Same in-memory state every time
- **Parallel**: Tests can run concurrently without conflicts

## CLI Features

### Standard Execution
Run `tixgraft` with a config file or CLI arguments to execute pull operations.

### To-Command-Line Conversion
The `--to-command-line` flag converts any configuration to an equivalent CLI command:
```bash
# Convert config to shell command
tixgraft --to-command-line

# Convert to JSON array
tixgraft --to-command-line --output-format json

# With CLI overrides
tixgraft --to-command-line --repository override/repo --tag v2.0
```

This is useful for:
- Generating shareable commands from complex configs
- Debugging configuration merging
- CI/CD integration (YAML → CLI)
- Documentation and examples

## Exit Codes

- 0: Success
- 1: Configuration error
- 2: Source error
- 3: Command error
- 4: Git error
- 5: Filesystem error

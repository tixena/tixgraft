# Testing Guide

This document describes the testing strategy for TixGraft and explains when to use `MockSystem` vs real filesystem operations.

## Test Categories

TixGraft has two types of tests with different requirements:

### 1. Unit Tests (Use MockSystem)

**Location:** `src/**/*.rs` (in `#[cfg(test)]` modules)
**Tool:** `MockSystem` (in-memory filesystem)
**Pattern:**
```rust
use crate::system::MockSystem;

let system = MockSystem::new()
    .with_env("MY_VAR", "value")
    .with_file("/test/config.yaml", content.as_bytes());

// Test library code directly
let result = my_function(&system, "/test/config.yaml");
```

**Examples:**
- `src/config/yaml.rs` - Config loading tests
- `src/config/validation.rs` - Validation logic tests
- `src/operations/replace.rs` - Text replacement tests

**Benefits:**
- No filesystem I/O = faster tests (milliseconds vs seconds)
- No temp directory cleanup needed
- Better isolation between tests
- Tests can run in parallel safely
- Works in environments with restricted filesystem access

### 2. Integration Tests (Use Real Filesystem)

**Location:** `tests/*.rs`
**Tool:** `TempDir` from `tempfile` crate
**Pattern:**
```rust
use tempfile::TempDir;
use assert_cmd::Command;

let temp_dir = TempDir::new().unwrap();
fs::write(temp_dir.path().join("config.yaml"), content).unwrap();

// Run the compiled binary as a subprocess
Command::cargo_bin("tixgraft")
    .current_dir(temp_dir.path())
    .arg("--config").arg("config.yaml")
    .env("MY_VAR", "value")  // Env vars for subprocess
    .assert()
    .success();

// Verify results on real filesystem
assert!(temp_dir.path().join("output.txt").exists());
```

**Examples:**
- `tests/cli_tests.rs` - CLI argument parsing
- `tests/context_tests.rs` - Context feature end-to-end
- `tests/local_tests.rs` - Local file:// repositories
- `tests/sparse_checkout_tests.rs` - Git sparse checkout
- `tests/to_command_line_tests.rs` - CLI conversion
- `tests/to_config_tests.rs` - Config generation

**Why Integration Tests MUST Use Real Filesystem:**

Integration tests use `Command::cargo_bin("tixgraft")` which runs the compiled binary as a separate subprocess. The subprocess:
1. Runs in its own process with its own memory space
2. Cannot access the parent process's in-memory `MockSystem`
3. Requires real files on disk to read
4. Writes output to the real filesystem
5. Receives environment variables via `.env()` method

**Attempting to use MockSystem in integration tests will cause:**
- "File not found" errors (subprocess can't see in-memory files)
- Test failures with no output (subprocess reads real filesystem)
- Confusion about test isolation

## File-by-File Breakdown

### Unit Test Files (Use MockSystem)

| File | Tests | Status |
|------|-------|--------|
| `tests/config_tests.rs` | 6 | ✅ Converted to MockSystem |

This is the ONLY test file that can use MockSystem because it tests library code directly via `Config::load_from_file(&system, path)`.

### Integration Test Files (Keep TempDir/fs::)

| File | Tests | Pattern | Why Real FS Required |
|------|-------|---------|----------------------|
| `tests/cli_tests.rs` | 8 | TempDir + Command | Binary needs real config files |
| `tests/context_tests.rs` | 13 | TempDir + Command + .env() | Binary reads .graft.yaml from disk |
| `tests/local_tests.rs` | 13 | TempDir + Command + .env() | Binary clones from file:// URLs |
| `tests/sparse_checkout_tests.rs` | 3 | TempDir + git commands | Real git repos required |
| `tests/to_command_line_tests.rs` | 11 | NamedTempFile + Command | Binary reads config files |
| `tests/to_config_tests.rs` | 13 | NamedTempFile + Command | Binary reads config files |

**Total:** 61 integration tests that CANNOT use MockSystem

## Making Code Testable with MockSystem

To make library code testable with MockSystem, ensure all filesystem and environment operations go through the `System` trait:

### Before (Not Testable)
```rust
pub fn load_config(path: &str) -> Result<Config> {
    // Direct filesystem access
    if !std::path::Path::new(path).exists() {
        return Err(...);
    }
    let content = fs::read_to_string(path)?;
    // ...
}
```

### After (Testable)
```rust
pub fn load_config(system: &dyn System, path: &str) -> Result<Config> {
    let path_obj = Path::new(path);

    // Use System trait
    if !system.exists(path_obj) {
        return Err(...);
    }
    let content = system.read_to_string(path_obj)?;
    // ...
}
```

### System Trait Operations

The `System` trait provides:

**Environment:**
- `env_var(key)` - Get environment variable
- `current_dir()` - Get current working directory

**Filesystem:**
- `read_to_string(path)` - Read file as string
- `write(path, contents)` - Write bytes to file
- `create_dir_all(path)` - Create directory and parents
- `remove_dir_all(path)` - Remove directory recursively
- `copy(from, to)` - Copy file
- `exists(path)` - Check if path exists
- `is_file(path)` - Check if path is a file
- `is_dir(path)` - Check if path is a directory
- `metadata(path)` - Get file metadata
- `canonicalize(path)` - Resolve to absolute path
- `read_dir(path)` - List directory entries
- `open(path)` - Open file for reading
- `create(path)` - Create file for writing

## Running Tests

```bash
# Run all tests
cargo test --all

# Run only unit tests (fast, use MockSystem where applicable)
cargo test --lib

# Run only integration tests (slower, use real filesystem)
cargo test --test '*'

# Run specific integration test file
cargo test --test cli_tests

# Run specific test
cargo test test_valid_basic_config
```

## Summary

- **Unit tests** (in `src/`): Use `MockSystem` for fast, isolated testing
- **Integration tests** (in `tests/`): Use `TempDir` + real filesystem because they run the compiled binary as a subprocess
- **Making code testable**: Accept `&dyn System` parameter instead of using `fs::` directly
- **Current status**: 6 unit tests use MockSystem, 61 integration tests use real filesystem

This architecture ensures:
1. Fast unit tests for library code (MockSystem)
2. Realistic end-to-end tests for binary behavior (real filesystem)
3. Clear separation of concerns between test types

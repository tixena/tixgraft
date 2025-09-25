# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2024-09-25

### Added

- Initial release of tixgraft CLI tool
- Git sparse checkout functionality for efficient repository fetching
- YAML configuration support with JSON schema validation
- Complete CLI interface with argument precedence over config files
- Text replacement engine with static values and environment variables
- Command execution with proper working directory context
- Comprehensive error handling with specific exit codes:
  - 0: Success
  - 1: Configuration Error
  - 2: Source Error  
  - 3: Command Error
  - 4: Git Error
  - 5: Filesystem Error
- Path validation and security features
- Support for multiple repository URL formats:
  - Short format: `org/repo`
  - HTTPS: `https://github.com/org/repo.git`
  - SSH: `git@github.com:org/repo.git`
  - Enterprise Git servers
- Dry run mode for previewing operations
- Binary file detection and skipping during text replacement
- Cross-platform compatibility (Linux, macOS, Windows)
- Comprehensive test suite with unit and integration tests
- Complete documentation with examples

### Requirements

- Git 2.25.0 or later for sparse checkout support
- Rust 1.70+ for building from source

### Known Limitations

- Text replacements use simple string matching (no regex support yet)
- No support for authentication with private repositories
- Commands are executed synchronously (no parallel execution)
- Limited progress indication for large operations

### Security

- Path traversal protection
- Safe command execution in target directory context
- Repository URL validation
- Environment variable validation before execution

[Unreleased]: https://github.com/tixena/tixgraft/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/tixena/tixgraft/releases/tag/v0.1.0

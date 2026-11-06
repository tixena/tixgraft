# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

### Added

- Initial release of tixgraft CLI tool
- Path-scoped Git fetching: only the requested path is written to disk
- YAML configuration support with JSON schema validation
- Complete CLI interface whose arguments override the config file's global settings
- Text replacement engine with static values and environment variables
- Command execution with proper working directory context
- Comprehensive error handling with specific exit codes:
  - 0: Success
  - 1: Configuration Error
  - 2: Source Error  
  - 3: Command Error
  - 4: Git Error
  - 5: Filesystem Error
  - 6: Skill Error
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

- Rust 1.98 or later for building from source
- A C compiler, `pkg-config` and the OpenSSL development headers for building from source

### Known Limitations

- Text replacements use simple string matching (no regex support yet)
- Private repositories authenticate only through an ssh-agent, a Git credential helper or credentials embedded in the URL
- Commands are executed synchronously (no parallel execution)
- Limited progress indication for large operations

### Security

- Path traversal protection
- Safe command execution in target directory context
- Repository URL validation
- Environment variable validation before execution

[Unreleased]: https://github.com/tixena/tixgraft/commits/master
[0.1.0]: https://github.com/tixena/tixgraft/releases/tag/v0.1.0

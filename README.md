# TixGraft

A CLI tool for fetching reusable components from Git repositories using sparse checkout. TixGraft enables developers to pull specific files or directories from Git repositories, apply text replacements, and execute post-processing commands—all configured through a YAML file or command-line arguments.

## Features

- 🚀 **Efficient Git Operations**: Uses sparse checkout to fetch only the files you need
- 📝 **YAML Configuration**: Comprehensive configuration with JSON schema validation
- 🔧 **CLI Interface**: Full command-line support with argument precedence over config files
- 🔄 **Text Replacement**: Replace placeholders with static values or environment variables  
- ⚡ **Command Execution**: Run commands after copying files with proper working directory context
- ✅ **Validation**: Comprehensive validation with clear error messages
- 🛡️ **Security**: Path validation prevents directory traversal attacks

## Installation

### Install from crates.io
```bash
cargo install tixgraft
```

### Install from GitHub
```bash
cargo install --git https://github.com/tixena/tixgraft
```

### Build from source
```bash
git clone https://github.com/tixena/tixgraft
cd tixgraft

# Using just (recommended)
just setup
just build-release

# Or using cargo directly
cargo build --release
```

## Quick Start

### 1. Create a `tixgraft.yaml` configuration file:

```yaml
# Global settings (optional)
repository: "myorg/scaffolds"  # Can be full URL or account/repo format
tag: "main"                    # Branch, tag, or commit hash

# Pull operations (required, minimum 1)
pulls:
  - source: "kubernetes/mongodb"    # Required
    target: "./k8s/mongodb"         # Required
    type: "directory"               # Optional, default: "directory"
    commands:                       # Optional, default: []
      - "kubectl apply -f ."
      - "echo 'MongoDB deployed'"
    replacements:                   # Optional, default: []
      - source: "{{NAMESPACE}}"
        target: "production"         # String literal
      - source: "{{REPLICAS}}"
        valueFromEnv: "REPLICA_COUNT" # Environment variable
```

### 2. Run tixgraft:

```bash
# Execute the configuration
tixgraft

# Preview what would be done
tixgraft --dry-run

# Use a different config file
tixgraft --config my-config.yaml
```

## Configuration Reference

### YAML Configuration Structure

```yaml
# Global Settings (both optional)
repository: "myorg/scaffolds"  # Repository URL or account/repo format
tag: "main"                    # Git reference (branch, tag, or commit)

# Pull Operations (required, minimum 1)
pulls:
  - source: "path/in/repo"      # Required: Source path in repository
    target: "./local/path"       # Required: Target path in workspace
    type: "directory"            # Optional: "file" or "directory" (default: "directory")
    repository: "override/repo"  # Optional: Override global repository
    tag: "v2.1.0"               # Optional: Override global tag
    reset: true                  # Optional: rm -rf target before copying (default: false)
    commands:                    # Optional: Commands to execute after copying
      - "npm install"
      - "npm run build"
    replacements:                # Optional: Text replacements
      - source: "{{PLACEHOLDER}}"
        target: "replacement"     # Static replacement
      - source: "{{ENV_VAR}}"
        valueFromEnv: "MY_VAR"   # From environment variable
```

### Repository URL Formats

- **Short format**: `myorg/repo` → `https://github.com/myorg/repo.git`
- **HTTPS**: `https://github.com/myorg/repo.git`
- **SSH**: `git@github.com:myorg/repo.git`
- **Enterprise**: `https://git.company.com/team/repo.git`

## Command-Line Interface

### Global Arguments

- `--repository <repo>`: Git repository URL or account/repo format
- `--tag <ref>`: Git reference (branch, tag, or commit hash)
- `--config <path>`: Alternative config file path (default: ./tixgraft.yaml)
- `--dry-run`: Preview operations without executing
- `--to-command-line`: Output the equivalent command-line invocation instead of executing
- `--output-format <format>`: Output format for --to-command-line: shell or json (default: shell)
- `--verbose`, `-v`: Enable verbose logging output
- `--help`, `-h`: Show help information
- `--version`: Show version

### Per-Pull Arguments (repeatable)

- `--pull-repository <repo>`: Repository for specific pull
- `--pull-tag <ref>`: Git reference for specific pull
- `--pull-type <type>`: Either "file" or "directory" (default: "directory")
- `--pull-source <path>`: Source path in Git repository
- `--pull-target <path>`: Target path in local workspace
- `--pull-reset`: For directories, rm -rf target before copying
- `--pull-commands <cmd1,cmd2,...>`: Comma-separated commands
- `--pull-replacement <SOURCE=TARGET>`: Text replacement (format: "SOURCE=TARGET" or "SOURCE=env:VAR")

### CLI-Only Usage

```bash
# Pull a single directory
tixgraft --repository myorg/templates --pull-source kubernetes/app --pull-target ./k8s

# Pull multiple items with different repositories
tixgraft \
  --pull-repository myorg/configs --pull-source docker/Dockerfile --pull-target ./Dockerfile --pull-type file \
  --pull-repository myorg/scripts --pull-source ci/deploy.sh --pull-target ./scripts/deploy.sh --pull-type file

# Text replacements via CLI
tixgraft \
  --repository myorg/templates \
  --pull-source kubernetes/app \
  --pull-target ./k8s \
  --pull-replacement "{{APP_NAME}}=my-app" \
  --pull-replacement "{{NAMESPACE}}=env:K8S_NAMESPACE"
```

## Converting Configuration to Command Line

TixGraft can convert any YAML configuration to an equivalent command-line invocation using the `--to-command-line` flag. This is useful for:

- **Sharing workflows**: Generate a single command others can run without needing a config file
- **Debugging configuration**: Verify how YAML config is interpreted and merged
- **CI/CD integration**: Convert human-friendly YAML to scriptable CLI commands
- **Documentation**: Show concrete examples of complex configurations

### Basic Usage

```bash
# Show the command-line equivalent of your config (default: shell format)
tixgraft --to-command-line

# Use a specific config file
tixgraft --config custom.yaml --to-command-line

# Output as JSON array (useful for programmatic consumption)
tixgraft --to-command-line --output-format json

# Apply CLI overrides before generating output
tixgraft --to-command-line --repository override/repo --tag v2.0
```

### Example

Given this `tixgraft.yaml`:
```yaml
repository: "myorg/templates"
tag: "v1.0.0"
pulls:
  - source: "kubernetes/base"
    target: "./k8s"
    reset: true
    replacements:
      - source: "{{APP_NAME}}"
        target: "my-app"
```

Running `tixgraft --to-command-line` outputs:
```bash
tixgraft \
  --repository "myorg/templates" \
  --tag "v1.0.0" \
  --pull-source "kubernetes/base" \
  --pull-target "./k8s" \
  --pull-reset \
  --pull-replacement "{{APP_NAME}}=my-app"
```

### Output Formats

- **shell** (default): Ready-to-execute shell command with proper escaping and line continuations
- **json**: JSON array of arguments, useful for programmatic processing

### Notes

- The generated command excludes the `--config` argument (it's config-free)
- Execution flags like `--dry-run` and `--verbose` are not included in the output
- All shell special characters are properly escaped for safe execution
- CLI argument overrides (--repository, --tag) are applied before generating the output

## Examples

### Basic Directory Copy

```yaml
repository: "myorg/templates"
tag: "main"
pulls:
  - source: "docker/nodejs"
    target: "./docker"
```

### Multi-Repository Setup

```yaml
pulls:
  - source: "configs/nginx"
    target: "./nginx"
    repository: "myorg/configs"
    tag: "v1.2.0"
  - source: "scripts/deploy.sh"
    target: "./deploy.sh"
    type: "file"
    repository: "myorg/scripts"
    tag: "latest"
```

### With Text Replacements

```yaml
repository: "myorg/k8s-templates"
pulls:
  - source: "apps/web-service"
    target: "./k8s/web"
    replacements:
      - source: "{{APP_NAME}}"
        target: "my-web-app"
      - source: "{{NAMESPACE}}"
        valueFromEnv: "K8S_NAMESPACE"
      - source: "{{IMAGE_TAG}}"
        valueFromEnv: "BUILD_TAG"
    commands:
      - "kubectl apply -f ."
```

### Complex Kubernetes Setup

```yaml
repository: "devops/k8s-scaffolds"
tag: "production"
pulls:
  - source: "base/namespace"
    target: "./k8s/00-namespace"
    replacements:
      - source: "{{NAMESPACE}}"
        valueFromEnv: "K8S_NAMESPACE"
  
  - source: "apps/mongodb"
    target: "./k8s/01-mongodb"
    reset: true
    replacements:
      - source: "{{STORAGE_CLASS}}"
        valueFromEnv: "STORAGE_CLASS"
      - source: "{{REPLICA_COUNT}}"
        target: "3"
    commands:
      - "kubectl apply -f ."
      - "kubectl wait --for=condition=ready pod -l app=mongodb --timeout=300s"
```

## Error Handling

TixGraft uses specific exit codes for different error types:

- **0**: Success - all operations completed successfully
- **1**: Configuration Error - missing or invalid configuration
- **2**: Source Error - source path not found in repository
- **3**: Command Error - one or more commands failed
- **4**: Git Error - Git operation failed
- **5**: Filesystem Error - file operation failed

## Requirements

- **Git**: Version 2.25.0 or later (for sparse checkout support)
- **Rust**: 1.70+ (for building from source)
- **Just**: (optional, for development) - Install from [just.systems](https://just.systems/)

## Security Considerations

- Path validation prevents directory traversal attacks
- Commands are executed in the target directory context
- Repository URLs are validated before use
- Binary files are skipped during text replacement

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Run the development pipeline:
   ```bash
   # Using just (recommended)
   just dev     # format, lint, and test
   just ci      # full CI pipeline
   
   # Or using cargo directly
   cargo test && cargo clippy
   ```
6. Submit a pull request

### Development Commands

This project uses [`just`](https://just.systems/) for task running. Install it and run:

```bash
just --list          # Show all available commands
just setup           # Set up development environment
just dev             # Quick development checks (format, lint, test)
just ci              # Full CI pipeline
just build-release   # Build optimized binary
just install         # Install from local source
just docs            # Generate documentation
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and changes.

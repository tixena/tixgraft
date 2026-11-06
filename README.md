# TixGraft

A CLI tool for fetching reusable components from Git repositories or local paths. TixGraft enables developers to pull specific files or directories, apply text replacements, and execute post-processing commands—all configured through a YAML file or command-line arguments.

## Features

- 🚀 **Efficient Git Operations**: Fetches a single commit of the requested branch or tag and writes only the path you asked for
- 📁 **Local Sources**: Pull from a folder on the same machine, with no Git involved
- 📝 **YAML Configuration**: Comprehensive configuration with JSON schema validation
- 🔧 **CLI Interface**: Full command-line support, with flags that override the config file's global settings
- 🎯 **Context System**: Define required properties in `.graft.yaml` files for parameterized components
- 🔄 **Text Replacement**: Replace placeholders with static values, environment variables, or context properties
- ⚡ **Command Execution**: Run commands after copying files with proper working directory context
- 🌳 **Child Configs**: A config can run further configs from its subdirectories
- ✅ **Validation**: Comprehensive validation with clear error messages including fail-fast context validation
- 🛡️ **Security**: Target path validation rejects absolute paths and `..`
- 🧩 **Claude Code Skill**: `--skill-install` writes a bundled skill that documents tixgraft for Claude Code

## Installation

### Install from crates.io
```bash
cargo install tixgraft
```

Installing builds from source and needs Rust 1.98 or later, a C compiler, `pkg-config` and the OpenSSL development headers:

```bash
# Debian and Ubuntu
sudo apt-get install build-essential pkg-config libssl-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config openssl-devel
```

### Install from GitHub
```bash
cargo install --git https://github.com/tixena/tixgraft
```

### Build from source
```bash
git clone https://github.com/tixena/tixgraft
cd tixgraft
cargo build --release
```

The binary is written to `target/release/tixgraft`.

## Quick Start

### 1. Create a `tixgraft.yaml` configuration file:

```yaml
# Global settings (optional)
repository: "my_organization/scaffolds"  # Can be full URL or account/repo format
tag: "main"                    # Branch, tag, or commit hash (default: "main")

# Pull operations (at least one pull or one child config is required)
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
# Global Settings (all optional)
repository: "my_organization/scaffolds"  # Repository URL, account/repo format, or file: path
tag: "main"                    # Git reference: branch, tag, or commit (default: "main")

# Pull Operations (required unless `children` is present)
pulls:
  - source: "path/in/repo"      # Required: Source path in repository
    target: "./local/path"       # Required: Target path, relative to this file's directory
    type: "directory"            # Optional: "file" or "directory" (default: "directory")
    repository: "override/repo"  # Optional: Override global repository
    tag: "v2.1.0"               # Optional: Override global tag
    reset: true                  # Optional: For directories, rm -rf target before copying (default: false)
    requireCleanTarget: true     # Optional: Fail if target has uncommitted Git changes (default: true)
    mustSucceed: true            # Optional: If false, a failure warns and the run continues (default: true)
    commands:                    # Optional: Commands to execute after copying
      - "npm install"
      - "npm run build"
    replacements:                # Optional: Text replacements
      - source: "{{PLACEHOLDER}}"
        target: "replacement"     # Static replacement
      - source: "{{ENV_VAR}}"
        valueFromEnv: "MY_VAR"   # From environment variable
    context:                      # Optional: Context values for .graft.yaml files
      projectName: "my-service"
      port: 8080
```

`target` cannot be absolute or contain `..`. `source` must name a path inside the repository: `.` does not select a whole Git repository. Keys the configuration does not define are ignored without an error, so a misspelled key takes no effect.

YAML anchors, aliases and merge keys (`<<`) work. A file that expands to more than 250,000 nodes, nests too deeply or holds more than one YAML document fails to parse.

Replacements apply to every non-binary file under the target, including files that were already there; [Security Considerations](#security-considerations) describes how a file counts as binary. A `valueFromEnv` variable must be set, or validation fails with exit code 1, also under `--dry-run`.

`requireCleanTarget` applies when the target already exists inside a Git working tree: modified or untracked files there, other than ignored files, stop the pull with exit code 5.

`mustSucceed: false` covers a failure while the pull runs, including a failing command or a target with uncommitted changes: the failure is logged as a warning and the run continues. It does not reach two cases: a pull with no repository stops the run with exit code 1, and the `requireCleanTarget` check under `--dry-run` stops it with exit code 5.

### Child Configs

A config can run other `tixgraft.yaml` files, alone or alongside its own pulls:

```yaml
repository: "my_organization/scaffolds"
tag: "main"

children:
  - "./services/api/tixgraft.yaml"
  - "./services/web/tixgraft.yaml"

processChildrenFirst: false      # Optional: Run children before this config's pulls (default: false)

pulls:
  - source: "shared/config"
    target: "./shared-config"
```

- Child paths must point into subdirectories: absolute paths and `..` are rejected
- Each child is independent and inherits no `repository`, `tag` or `context`, including values given on the command line
- A child's `target` paths resolve against the child's own directory
- Children can nest up to 11 levels deep

### Context System

TixGraft supports a powerful context system that allows components to define required properties via `.graft.yaml` files. This enables parameterized, reusable components with validation.

#### Context in tixgraft.yaml

Context can be defined at the root level (shared by all pulls) or per-pull:

```yaml
repository: "my_organization/service-templates"

# Global context (available to all pulls)
context:
  organization: "myorg"
  environment: "production"

pulls:
  - source: "services/api"
    target: "./api"
    type: "directory"
    context:  # Pull-specific context (merged with global)
      serviceName: "user-api"
      port: 8080
```

#### Context via CLI

Context values can also be provided via command-line arguments:

```bash
# Simple key=value format
tixgraft --context projectName=MyApp --context port=8080

# JSON format for complex values (arrays, objects)
tixgraft --context-json services='[{"name":"api","port":8080}]'
```

Context hierarchy: Pull config > CLI arguments > Root config

#### .graft.yaml Files

Components can include a `.graft.yaml` file that defines:
- Required and optional context properties with type validation
- Text replacements using context values
- Post-processing commands

Example `.graft.yaml`:

```yaml
# Define required context properties
context:
  - name: serviceName
    description: "The name of the service"
    dataType: string

  - name: port
    description: "Service port number"
    dataType: number
    defaultValue: 8080

  - name: replicas
    description: "Number of replicas"
    dataType: number
    defaultValue: 3

# Text replacements using context
replacements:
  - source: "{{SERVICE_NAME}}"
    valueFromContext: serviceName
  - source: "{{PORT}}"
    valueFromContext: port
  - source: "{{REPLICAS}}"
    valueFromContext: replicas

# Post-processing commands
postCommands:
  - command: kubectl
    args: ["apply", "-f", "."]
```

**Supported Data Types:**
- `string`: Text values
- `number`: Integer or floating-point numbers
- `boolean`: true/false values
- `array`: Arrays of any type (use `--context-json` for complex arrays)

Each property needs `name`, `description` and `dataType`. A `defaultValue` makes it optional.

**Type Coercion:** String values are automatically coerced to the target type when possible:
- `"true"`, `"yes"`, `"1"` → `true` (boolean); `"false"`, `"no"`, `"0"` → `false` (boolean)
- `"8080"` → `8080` (number)

**Validation:** TixGraft validates context properties fail-fast:
- Missing required properties → Error with exit code 1
- Invalid types that can't be coerced → Error with exit code 1
- Properties not defined in context → Ignored

**Post-Commands:** Each entry runs `command` directly with its `args`, without a shell. A file's own replacements do not reach its `args`, but the pull's `replacements` and those of any `.graft.yaml` above it rewrite the file before it is read, so their placeholders are expanded there. An entry runs in the directory holding its `.graft.yaml`, or in `cwd` relative to that directory. A failing post-command logs a warning: later entries still run and the pull still succeeds. An entry can also be conditional, running the `onMatch` command of the first option whose `test` output matches the `expectedOutput` regular expression:

```yaml
postCommands:
  - type: choice
    options:
      - test:
          command: node
          args: ["--version"]
        expectedOutput: "^v2\\d"
        onMatch:
          command: npm
          args: ["install"]
```

**Processing Flow:**
1. Context is validated against the requirements of every `.graft.yaml` in `source`, before the target is changed
2. Files are copied to target directory
3. The pull's own `replacements` are applied
4. `.graft.yaml` files are discovered recursively within the copied target, root first
5. Each file's text replacements are applied to its directory and everything beneath it, using context values
6. Each file's post-commands are executed
7. `.graft.yaml` files are cleaned up
8. The pull's own `commands` are executed

Discovery covers only `source`. A `.graft.yaml` sitting above `source` in the repository is never
read or applied: its context goes unvalidated and its replacements are skipped. Pull a component at
the root where its `.graft.yaml` lives, not at a subfolder beneath it.

### Repository URL Formats

- **Short format**: `my_organization/repo` → `https://github.com/my_organization/repo.git`
- **HTTPS**: `https://github.com/my_organization/repo.git` (the `.git` suffix is required)
- **SSH**: `git@github.com:my_organization/repo.git` (`ssh://` URLs are not accepted)
- **Enterprise**: `https://git.company.com/team/repo.git`
- **Local**: `file:~/scaffolds`, see [Local Sources](#local-sources)

When no `tag` is given, TixGraft asks for `main`, whatever the repository's default branch is.

A branch or tag name fetches that single commit. A commit hash fetches every branch and tag with their history. Either way the whole tree of the commit is downloaded, and only `source` is written to disk.

### Local Sources

A repository starting with `file:`, `~`, `/`, `./` or `../` is a local folder. TixGraft copies straight from it without any Git operation and ignores `tag`.

```yaml
repository: "file:~/scaffolds"
pulls:
  - source: "templates/service"
    target: "./service"
```

- YAML accepts only the `file:` forms: `file:/abs/path`, `file:///abs/path`, `file:~/path`, `file:relative/path`
- The command line also accepts bare paths: `~/path`, `/abs/path`, `./rel`, `../rel`
- A missing folder, or a path that is not a folder, is a source error with exit code 2

Directory copies from any source skip files matched by `.ignore` files. Git sources, and local folders inside a Git repository, also skip files matched by `.gitignore` rules and your global Git excludes. Dotfiles are copied, including a `.git` directory when `source` contains one. A symbolic link to a file is copied as a regular file, and a symbolic link to a directory becomes an empty directory.

### Authentication

Credentials come from your existing SSH and Git setup. TixGraft has no dedicated flag, config key or environment variable for them, and it never prompts.

- **SSH**: Keys come only from a running ssh-agent, found through `SSH_AUTH_SOCK`. Key files in `~/.ssh` are not read, so load them with `ssh-add`. The server's host key must already be in `~/.ssh/known_hosts`.
- **HTTPS**: Public repositories need nothing. When the server asks for authentication, TixGraft calls the `credential.helper` from your Git configuration. A helper set by name, such as `store`, runs `git credential-<name>` and needs `git` on the `PATH`. A username or password embedded in the URL takes precedence over the helper, and TixGraft prints that URL in its logs and error messages.

A failed authentication is a Git error with exit code 4.

## Command-Line Interface

### Global Arguments

- `--repository <repo>`: Git repository URL, account/repo format, or local path. Overrides the config's global `repository`; a pull's own `repository` still wins
- `--tag <ref>`: Git reference (branch, tag, or commit hash). Overrides the config's global `tag`; a pull's own `tag` still wins
- `--config <path>`: Alternative config file path (default: ./tixgraft.yaml)
- `--context <KEY=VALUE>`: Context values in KEY=VALUE format (repeatable, multiple values with same key create array)
- `--context-json <KEY=JSON>`: Context values as JSON for complex types like arrays or objects (repeatable)
- `--dry-run`: Preview operations without executing. Validates the config and checks that targets are clean, without contacting the repository, copying files or running commands
- `--to-command-line`: Output the command-line form of the config file instead of executing
- `--to-config`: Output the YAML form of the given flags instead of executing
- `--output-format <format>`: Output format for --to-command-line: shell or json (default: shell)
- `--verbose`, `-v`: Enable verbose logging output
- `--help`, `-h`: Show help information
- `--version`, `-V`: Show version

### Per-Pull Arguments (repeatable)

Flags pair up by position: the second `--pull-type` belongs to the second `--pull-source`, so a flag given once applies to the first pull. `--pull-replacement` is the exception, as every replacement applies to every pull given on the command line. When `--pull-source` is present, the pulls on the command line replace all pulls in the config file.

- `--pull-repository <repo>`: Repository for specific pull
- `--pull-tag <ref>`: Git reference for specific pull
- `--pull-type <type>`: Either "file" or "directory" (default: "directory")
- `--pull-source <path>`: Source path in Git repository or local folder
- `--pull-target <path>`: Target path in local workspace
- `--pull-reset <true|false>`: For directories, rm -rf target before copying (default: false)
- `--pull-require-clean-target <true|false>`: Fail if target has uncommitted Git changes (default: true)
- `--pull-must-succeed <true|false>`: If false, a failure warns and the run continues (default: true)
- `--pull-commands <cmd1,cmd2,...>`: Comma-separated commands
- `--pull-replacement <SOURCE=TARGET>`: Text replacement (format: "SOURCE=TARGET" or "SOURCE=env:VAR")

### Skill Management

TixGraft bundles a Claude Code skill that documents these features:

- `--skill-install`: Install the skill to `./.claude/skills/tixgraft/`
- `--skill-uninstall`: Remove the skill
- `--skill-test`: Report whether the skill is installed and up to date, and offer to install or upgrade it. Exits with 1 if you decline the install and 2 if you decline the upgrade
- `--global`, `-g`: Use `~/.claude/skills/tixgraft/` instead
- `--yes`, `-y`: Accept the `--skill-test` offer without asking

### CLI-Only Usage

```bash
# Pull a single directory
tixgraft --repository my_organization/templates --pull-source kubernetes/app --pull-target ./k8s

# Pull multiple items with different repositories
tixgraft \
  --pull-repository my_organization/configs --pull-source docker/Dockerfile --pull-target ./Dockerfile --pull-type file \
  --pull-repository my_organization/scripts --pull-source ci/deploy.sh --pull-target ./scripts/deploy.sh --pull-type file

# Text replacements via CLI
tixgraft \
  --repository my_organization/templates \
  --pull-source kubernetes/app \
  --pull-target ./k8s \
  --pull-replacement "{{APP_NAME}}=my-app" \
  --pull-replacement "{{NAMESPACE}}=env:K8S_NAMESPACE"
```

## Converting Configuration to Command Line

TixGraft can convert a YAML configuration file to a command-line invocation using the `--to-command-line` flag. This is useful for:

- **Sharing workflows**: Generate a single command others can run without needing a config file
- **Debugging configuration**: Verify how YAML config is interpreted and merged
- **CI/CD integration**: Convert human-friendly YAML to scriptable CLI commands
- **Documentation**: Show concrete examples of complex configurations

### Basic Usage

```bash
# Show the command-line form of your config (default: shell format)
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
repository: "my_organization/templates"
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
  --repository \
  my_organization/templates \
  --tag \
  v1.0.0 \
  --pull-source \
  kubernetes/base \
  --pull-target \
  ./k8s \
  --pull-reset \
  true \
  --pull-replacement \
  "{{APP_NAME}}=my-app"
```

### Output Formats

- **shell** (default): Shell command with line continuations
- **json**: JSON array of arguments, useful for programmatic processing

### Notes

- A config file is required: `--pull-*` flags are not read in this mode
- The generated command excludes the `--config` argument (it's config-free)
- Execution flags like `--dry-run` and `--verbose` are not included in the output
- Arguments with special characters are wrapped in double quotes, with `"`, `\`, `$`, `` ` `` and `!` backslash-escaped
- CLI argument overrides (--repository, --tag) are applied before generating the output
- The output carries `repository`, `tag` and `pulls` only: `context`, `children` and `processChildrenFirst` are left out
- Child paths are checked against the working directory, not the config file's directory, so convert a config with `children` from its own directory: elsewhere a child that is not found there fails the conversion with exit code 1
- Per-pull flags pair up by position and are emitted only when set, so the output matches the config only for a single pull with at most one command. With more pulls or commands, flags shift to the wrong pull or are dropped: review the output before running it

### Converting Command Line to YAML

`--to-config` prints the YAML configuration for a set of flags, merged over `--config` when that file exists:

```bash
tixgraft --to-config --repository my_organization/templates --pull-source kubernetes/app --pull-target ./k8s
```

`--context` values appear in the output only when a config file was loaded. Give local sources in the `file:` form, since a bare path in the output fails validation when the file is loaded. A config with `children` must be converted from its own directory, as with `--to-command-line`.

## Examples

### Basic Directory Copy

```yaml
repository: "my_organization/templates"
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
    repository: "my_organization/configs"
    tag: "v1.2.0"
  - source: "scripts/deploy.sh"
    target: "./deploy.sh"
    type: "file"
    repository: "my_organization/scripts"
    tag: "latest"
```

### With Text Replacements

```yaml
repository: "my_organization/k8s-templates"
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

### Using Context for Parameterized Components

When pulling components that include `.graft.yaml` files:

```yaml
# tixgraft.yaml
repository: "my_organization/service-templates"
context:
  organization: "mycompany"
  environment: "production"

pulls:
  - source: "microservices/api-service"
    target: "./services/user-api"
    context:
      serviceName: "user-api"
      port: 8080
      replicas: 3
```

The pulled component's `.graft.yaml` might look like:

```yaml
# microservices/api-service/.graft.yaml (in the repository)
context:
  - name: organization
    description: "Owning organization"
    dataType: string
  - name: serviceName
    description: "Name of the microservice"
    dataType: string
  - name: port
    description: "Service port"
    dataType: number
  - name: replicas
    description: "Number of replicas"
    dataType: number
    defaultValue: 2

replacements:
  - source: "{{ORGANIZATION}}"
    valueFromContext: organization
  - source: "{{SERVICE_NAME}}"
    valueFromContext: serviceName
  - source: "{{PORT}}"
    valueFromContext: port
  - source: "{{REPLICAS}}"
    valueFromContext: replicas

postCommands:
  - command: kubectl
    args: ["apply", "-f", "."]
```

You can also provide context via CLI, for properties the pull does not set itself:

```bash
tixgraft --context serviceName=user-api --context port=8080 --context replicas=3
```

### Complex Kubernetes Setup

```yaml
repository: "my_organization/k8s-scaffolds"
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

- **0**: Success - the run completed. Failures in pulls with `mustSucceed: false` and failing `.graft.yaml` post-commands only log warnings
- **1**: Configuration Error - missing or invalid configuration or context, and any failure without a code of its own, such as a permission error while copying
- **2**: Source Error - source path not found in repository, not of the declared type, or empty; also a missing local folder. The argument parser exits with 2 as well on invalid command-line usage
- **3**: Command Error - a command from `commands` failed
- **4**: Git Error - Git operation failed, including authentication and unknown references
- **5**: Filesystem Error - target has uncommitted changes, or a target or working directory is missing
- **6**: Skill Error - a `--skill-*` operation failed

## Requirements

- **Rust**: 1.98 or later, with the native prerequisites listed under [Installation](#installation) (for building)
- **Git**: Pulling links libgit2 and needs no `git` executable, unless a `credential.helper` is set by name

## Security Considerations

- `target` paths are validated: absolute paths and `..` are rejected. `source` paths are not, so a local `source` containing `..` reads outside the local folder
- Pulling runs code: `commands` and the `postCommands` of every pulled `.graft.yaml` execute with your permissions. `--dry-run` lists only `commands`, so inspect sources you do not trust
- Commands are executed in the target directory, or in the target's parent directory for a `file` pull
- Repository URLs are validated before use
- Binary files are skipped during text replacement. Detection reads only the first 8192 bytes of a file without a known text extension such as `.txt`: a null byte or invalid UTF-8 there marks the file binary. A multibyte character cut by the 8192-byte limit does not count, so valid text with one there is still replaced. A file that passes detection but is not valid UTF-8 throughout, including any file with a known text extension, fails the pull with exit code 1

## Contributing

Issues and pull requests are welcome at [github.com/tixena/tixgraft](https://github.com/tixena/tixgraft). Before submitting, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

## License

Licensed under the [MIT License](LICENSE).

## Changelog

See [CHANGELOG.md](https://github.com/tixena/tixgraft/blob/master/CHANGELOG.md) for version history and changes.

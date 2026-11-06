---
name: tixgraft
description: Guide for using tixgraft (also called "graft") to pull or graft reusable components from Git repositories or local filesystem paths, using YAML config, CLI arguments, text replacements, and context-driven templating.
user-invocable: true
---

# tixgraft (graft)

tixgraft is a CLI tool for fetching reusable components from Git repositories or from local filesystem paths (a `file:` source). When users say "graft" they mean tixgraft — e.g., "graft a component from github", "I want to graft this into my project", "pull this scaffold using graft".

## When to Use

Use tixgraft when the user wants to:
- Pull specific files or directories from a remote Git repository (or a local folder) into their project
- Scaffold or template new components from a shared repository
- Apply text replacements (placeholders) on pulled content
- Run post-processing commands after pulling files
- Reuse infrastructure configs, service templates, or any shareable code from Git

## CLI Reference

### Core Flags

```
tixgraft                              # Run with ./tixgraft.yaml config
tixgraft --config <path>              # Use a specific config file
tixgraft --repository <repo>          # Repository (overrides the config's global value)
tixgraft --tag <ref>                  # Branch, tag, or commit (overrides the config's global value; default: main)
tixgraft --dry-run                    # Validate and preview, without contacting the repository or changing files
tixgraft --verbose / -v               # Debug logging
tixgraft --to-command-line            # Convert config to CLI command
tixgraft --to-config                  # Convert CLI args to YAML config
tixgraft --output-format <fmt>        # "shell" (default) or "json" for --to-command-line
```

### Per-Pull Flags (repeatable, index-aligned)

Each `--pull-*` flag at index N pairs with other `--pull-*` flags at the same index, so a flag given once applies to the first pull:

```
--pull-source <path>              # Source path within the repository or local folder (required)
--pull-target <path>              # Target path in local workspace (required)
--pull-type <type>                # "file" or "directory" (default: "directory")
--pull-repository <repo>          # Override repository for this pull
--pull-tag <ref>                  # Override tag for this pull
--pull-reset <true|false>         # For directories, rm -rf target before copying (default: false)
--pull-require-clean-target <true|false>  # require clean git target (default: true)
--pull-must-succeed <true|false>  # whether failure is fatal (default: true)
--pull-commands <cmds>            # Post-copy commands (comma-separated)
--pull-replacement <SRC=TGT>      # Text replacement: "{{PLACEHOLDER}}=value" or "{{VAR}}=env:ENV_NAME"
```

When `--pull-source` is present, the pulls on the command line replace all pulls in the config file.

**Note on `--pull-replacement`:** unlike the other `--pull-*` flags, replacements are **not** index-aligned. Every `--pull-replacement` given on the command line is applied to **all** CLI pulls (a known CLI limitation). Use a YAML config when you need per-pull replacements.

### Context Flags

```
--context <KEY=VALUE>             # Simple context value (repeatable, same key creates array)
--context-json <KEY=JSON>         # Complex context as JSON (arrays, objects)
```

### Skill Management Flags

```
--skill-install                   # Install the tixgraft Claude Code skill (project-scoped)
--skill-install -g                # Install globally (~/.claude/skills/tixgraft/)
--skill-uninstall                 # Remove the skill (project-scoped)
--skill-uninstall -g              # Remove globally
--skill-test                      # Check if skill is installed and up to date (interactive)
--skill-test -y                   # Auto-confirm prompts
--skill-test -g                   # Check global installation
```

`--skill-test` exits with 0 when the skill is up to date or was just installed, 1 when an install is declined, and 2 when an upgrade is declined.

## Repository URL Formats

tixgraft accepts these repository formats:

| Format | Example | Expands To |
|--------|---------|------------|
| Short | `my_org/repo` | `https://github.com/my_org/repo.git` |
| HTTPS | `https://github.com/my_org/repo.git` | (used as-is; the `.git` suffix is required) |
| SSH | `git@github.com:my_org/repo.git` | (used as-is; `ssh://` URLs are not accepted) |
| Local | `file:///abs/path`, `file:~/rel/to/home`, `~/rel/to/home`, `/abs/path`, `./rel`, `../rel` | local folder (no Git — see below) |

Enterprise Git hosts work with full HTTPS/SSH URLs.

A branch or tag name fetches that single commit. A commit hash fetches every branch and tag with their history. Either way the whole tree of the commit is downloaded, and only `source` is written to disk. `source` must name a path inside the repository: `.` does not select a whole Git repository.

## Authentication

Credentials come from the user's existing SSH and Git setup. tixgraft has no dedicated flag, config key or environment variable for them, and it never prompts. A failed authentication exits with code 4.

- **SSH**: keys come only from a running ssh-agent, found through `SSH_AUTH_SOCK`. Key files in `~/.ssh` are not read, so load them with `ssh-add`. The server's host key must already be in `~/.ssh/known_hosts`.
- **HTTPS**: public repositories need nothing. When the server asks for authentication, tixgraft calls the `credential.helper` from the Git configuration. A helper set by name, such as `store`, runs `git credential-<name>` and needs `git` on the `PATH`. Pulling otherwise needs no `git` executable. A username or password embedded in the URL takes precedence over the helper, and tixgraft prints that URL in its logs and error messages.

## Local (Filesystem) Sources

Besides Git, tixgraft can pull from a **local folder** on the same machine. A repository given as a `file:` URL **or, on the command line, a plain filesystem path** is treated as a local source: tixgraft **skips Git entirely** and copies straight from the local path.

### Accepted forms

| Form | Meaning |
|------|---------|
| `file:///abs/path` | Absolute path (`file://` stripped → `/abs/path`) |
| `file:/abs/path` | Absolute path (`file:` stripped → `/abs/path`) |
| `file:~/rel/to/home` | Leading `~` expands to `$HOME` (or `$USERPROFILE`) |
| `file:relative/path` | Resolved against the current working directory |
| `~/rel/to/home` | Bare tilde path (`~` expands to `$HOME` / `$USERPROFILE`) |
| `/abs/path` | Bare absolute path |
| `./rel`, `../rel` | Bare relative path, resolved against the current working directory |

A source is treated as **local** when it starts with `file:`, `~`, `/`, `./`, or `../`. Everything else — the short `org/repo` form and HTTP(S)/SSH URLs — is treated as **Git**. (A plain relative path with no leading `./` — e.g. `foo/bar` — is the short Git form, not a local path; prefix it with `./` to force local.)

**A YAML config accepts only the `file:` forms.** The bare forms work with `--repository` and `--pull-repository`; in YAML they fail schema validation with exit code 1.

### Semantics

- **No Git**: local sources never clone or check out. The `tag` / `--tag` value is ignored for a local source.
- **Validation**: the resolved path must **exist** and be a **directory**, otherwise tixgraft fails with a source error (exit code 2). The per-pull `type` (`file` or `directory`) is still checked against the resolved `source` inside that folder.
- **`~` expansion**: a leading `~` (with or without the `file:` prefix) expands to the home directory before resolving.
- **Copy is ignore-aware**: directory copies walk the source with the Rust `ignore` crate (the same traversal ripgrep uses). Files matched by `.gitignore` rules or the user's global Git excludes (applied when the source lives inside a Git repository, as every Git source does) or by `.ignore` files are **skipped**, not copied. Dotfiles are copied, including a `.git` directory when `source` contains one. A symlink to a file is copied as a regular file, and a symlink to a directory becomes an empty directory.

### Example

```yaml
# tixgraft.yaml — pull from a local scaffold folder instead of a remote repo
repository: "file:~/scaffolds"
pulls:
  - source: "templates/service"
    target: "./service"
```

```bash
# Same idea, entirely from the CLI
tixgraft \
  --repository file:~/scaffolds \
  --pull-source templates/service \
  --pull-target ./service
```

## YAML Configuration

The default config file is `./tixgraft.yaml`. Structure:

```yaml
# Global settings (optional, can be overridden per-pull)
repository: "my_org/scaffolds"
tag: "main"

# Context values (optional, available to all pulls)
context:
  organization: "mycompany"
  environment: "production"

# Pull operations (required unless 'children' is present)
pulls:
  - source: "path/in/repo"         # Required
    target: "./local/path"          # Required
    type: "directory"               # Optional: "file" or "directory" (default: "directory")
    repository: "other/repo"        # Optional: override global
    tag: "v1.0.0"                   # Optional: override global (default: "main")
    reset: true                     # Optional: for directories, delete target first
    requireCleanTarget: false       # Optional: skip uncommitted changes check (default: true)
    mustSucceed: false              # Optional: if false, failure warns instead of aborting (default: true)
    context:                        # Optional: per-pull context (merged with global)
      serviceName: "my-api"
      port: 8080
    replacements:                   # Optional
      - source: "{{PLACEHOLDER}}"
        target: "value"             # Static replacement
      - source: "{{VAR}}"
        valueFromEnv: "ENV_NAME"    # From environment variable
    commands:                       # Optional: run after copying
      - "npm install"
      - "npm run build"
```

**Config hierarchy**: per-pull config > CLI `--repository` / `--tag` > global config.

**Target paths** must be relative (no leading `/`, no `..`); they resolve against the config file's directory. A relative `file:` repository still resolves against the current working directory, so a config using one reads a different folder, or fails, when run from elsewhere.

**`requireCleanTarget`** applies when the target already exists inside a Git working tree: modified or untracked files there, other than ignored files, stop the pull with exit code 5. `--dry-run` runs the same check.

**`mustSucceed: false`** covers a failure while the pull runs, including a failing command or a target with uncommitted changes: the failure is logged as a warning and the run continues with exit code 0. It does not reach two cases: a pull with no repository stops the run with exit code 1, and the `requireCleanTarget` check under `--dry-run` stops it with exit code 5.

Keys the configuration does not define are ignored without an error, so a misspelled key takes no effect.

YAML anchors, aliases and merge keys (`<<`) work. A file that expands to more than 250,000 nodes, nests too deeply or holds more than one YAML document fails to parse.

### Children (Cascading Execution)

A config can reference child `tixgraft.yaml` files for cascading execution:

```yaml
# Pure orchestrator -- no pulls, just children
children:
  - "./services/api/tixgraft.yaml"
  - "./services/web/tixgraft.yaml"
```

```yaml
# Mixed -- own pulls plus children
repository: "my_org/scaffolds"
tag: "main"

children:
  - "./services/api/tixgraft.yaml"

# processChildrenFirst: true  # Run children before parent pulls

pulls:
  - source: "shared/config"
    target: "./shared-config"
```

**Rules:**
- Children must be in subdirectories (no `..`, no absolute paths)
- Children are fully independent -- no inheritance of `repository`, `tag`, or `context`
- Max nesting depth: 11
- A config must have `pulls` or `children` (or both)
- Default execution order: parent pulls first, then children
- `processChildrenFirst: true` reverses the order

## Text Replacements

Replacements find-and-replace text in all non-binary files under the target after copying, including files that were already there. A static value in a pull's `replacements` cannot be empty; in a `.graft.yaml` an empty `target` is accepted and removes the placeholder.

Binary detection reads only the first 8192 bytes of a file without a known text extension such as `.txt`: a null byte or invalid UTF-8 there marks the file binary and skips it. A multibyte character cut by the 8192-byte limit does not count, so valid text with one there is still replaced. A file that passes detection but is not valid UTF-8 throughout, including any file with a known text extension, fails the pull with exit code 1.

### In YAML

```yaml
replacements:
  - source: "{{APP_NAME}}"
    target: "my-app"               # Static value
  - source: "{{NAMESPACE}}"
    valueFromEnv: "K8S_NAMESPACE"  # Read from environment variable
```

### Via CLI

```bash
--pull-replacement "{{APP_NAME}}=my-app"
--pull-replacement "{{NAMESPACE}}=env:K8S_NAMESPACE"
```

The `env:` prefix tells tixgraft to read from an environment variable.

## Context System and .graft.yaml

Components in the source repository can include `.graft.yaml` files that define required properties, replacements, and post-commands. This enables parameterized, reusable components.

### How .graft.yaml Works

A `.graft.yaml` file in the source repository defines:

```yaml
# Required and optional context properties
context:
  - name: serviceName
    description: "Name of the service"
    dataType: string              # string, number, boolean, array
  - name: port
    description: "Service port"
    dataType: number
    defaultValue: 8080            # Optional default

# Replacements using context values
replacements:
  - source: "{{SERVICE_NAME}}"
    valueFromContext: serviceName
  - source: "{{PORT}}"
    valueFromContext: port

# Commands to run after processing
postCommands:
  - command: echo
    args: ["Service configured"]
```

Each `.graft.yaml` replacement must specify **exactly one** value source:

- `target:` — a static literal value
- `valueFromEnv:` — read from an environment variable
- `valueFromContext:` — read from a context property (shown above)

Each `context` entry needs `name`, `description` and `dataType`; a `defaultValue` makes it optional.

`postCommands` entries also accept an optional `cwd:` (working directory relative to the graft) and support a conditional `choice` form (`type: choice` with `options`, each running a `test` command and executing `onMatch` when the output matches the `expectedOutput` regular expression; the first match wins). A `postCommand` with no `type` defaults to `type: command`.

A post-command runs directly, without a shell. A file's own replacements do not reach its `args`, but the pull's `replacements` and those of any `.graft.yaml` above it rewrite the file before it is read, so their placeholders are expanded there. A failing post-command only logs a warning: later entries still run and the pull still succeeds.

### Providing Context

Context values can come from three sources (in priority order):

1. **Per-pull config**: `context:` block under a pull
2. **CLI**: `--context serviceName=my-api --context port=8080`
3. **Global config**: `context:` block at root level

For complex values use `--context-json`:
```bash
--context-json 'services=[{"name":"api","port":8080}]'
```

### Type Coercion

String values are automatically coerced to the declared type:
- `"true"`, `"yes"`, `"1"` -> boolean `true`; `"false"`, `"no"`, `"0"` -> boolean `false`
- `"8080"` -> number `8080`

An **empty-string** context value unsets a property, and where it is set decides the result. In a pull's `context`, it removes the value inherited from the global context, so a declared property falls back to its `defaultValue`, or fails as a missing required property with exit code 1 when it has none. At root level or through `--context`, it leaves a declared property unset: its `defaultValue` is not applied and validation passes, but a replacement that reads it fails with exit code 1.

### Validation

- Missing required properties (no default) -> exit code 1
- Invalid types that can't be coerced -> exit code 1
- Extra properties not in the context definition -> ignored

### Processing Flow

1. Context is validated against every `.graft.yaml` in `source`, before the target is changed
2. Files are copied to target
3. The pull's own `replacements` are applied
4. `.graft.yaml` files are discovered recursively in target, root first
5. Each file's replacements are applied to its directory and everything beneath it, using context values
6. Each file's post-commands execute
7. `.graft.yaml` files are cleaned up (removed from target)
8. The pull's own `commands` execute

Discovery covers only `source`. A `.graft.yaml` above `source` in the repository is never read or
applied — its context goes unvalidated and its replacements are skipped. Pull a component at the
root where its `.graft.yaml` lives, not at a subfolder beneath it.

## Config-to-CLI Conversion

Convert a YAML config file to a CLI command:

```bash
tixgraft --to-command-line                    # Shell format
tixgraft --to-command-line --output-format json  # JSON array
tixgraft --to-command-line --repository override/repo  # With overrides
```

The output carries `repository`, `tag` and `pulls` only: `context`, `children` and `processChildrenFirst` are left out. Per-pull flags pair up by index and are emitted only when set, so the output matches the config only for a single pull with at most one command. With more pulls or commands, flags shift to the wrong pull or are dropped: review the output before running it. Child paths are checked against the working directory, not the config file's directory, so convert a config with `children` from its own directory: elsewhere a child that is not found there fails the conversion with exit code 1.

Convert CLI args to a YAML config:

```bash
tixgraft --to-config --repository my_org/repo --pull-source src --pull-target ./dest
```

`--context` values appear in the output only when a config file was loaded. Give local sources in the `file:` form, since a bare path in the output fails validation when the file is loaded. A config with `children` must be converted from its own directory, as with `--to-command-line`.

## CLI-Only Usage (No Config File)

You can use tixgraft entirely from the command line without a YAML file:

```bash
# Pull a single directory
tixgraft --repository my_org/templates --pull-source kubernetes/app --pull-target ./k8s

# Pull a single file
tixgraft --repository my_org/configs --pull-source docker/Dockerfile --pull-target ./Dockerfile --pull-type file

# Multiple pulls from different repos
tixgraft \
  --pull-repository my_org/configs --pull-source nginx/default.conf --pull-target ./nginx.conf --pull-type file \
  --pull-repository my_org/scripts --pull-source ci/deploy.sh --pull-target ./deploy.sh --pull-type file

# With replacements and commands
tixgraft \
  --repository my_org/templates \
  --pull-source kubernetes/app \
  --pull-target ./k8s \
  --pull-replacement "{{APP_NAME}}=my-app" \
  --pull-replacement "{{NAMESPACE}}=env:K8S_NAMESPACE" \
  --pull-commands "kubectl apply -f ."

# With context for .graft.yaml processing
tixgraft \
  --repository my_org/service-templates \
  --pull-source microservices/api \
  --pull-target ./services/my-api \
  --context serviceName=my-api \
  --context port=8080
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (failures under `mustSucceed: false` and failing post-commands only warn) |
| 1 | Configuration error, and any failure without a code of its own, such as a permission error while copying |
| 2 | Source error (path not found in repo, wrong type, empty, or missing local folder); also invalid command-line usage |
| 3 | Command error (a command from `commands` failed) |
| 4 | Git error (fetch, authentication or unknown reference) |
| 5 | Filesystem error (target has uncommitted changes, or a target or working directory is missing) |
| 6 | Skill error |

## Common Workflows

### Scaffold a new service from a template repo

```bash
tixgraft --repository my_org/service-templates \
  --pull-source microservices/api-service \
  --pull-target ./services/user-api \
  --context serviceName=user-api --context port=8080
```

### Pull shared infrastructure configs

```yaml
# tixgraft.yaml
repository: "my_org/k8s-scaffolds"
tag: "production"
pulls:
  - source: "base/namespace"
    target: "./k8s/namespace"
    replacements:
      - source: "{{NAMESPACE}}"
        valueFromEnv: "K8S_NAMESPACE"
  - source: "apps/mongodb"
    target: "./k8s/mongodb"
    reset: true
    commands:
      - "kubectl apply -f ."
  - source: "extras/monitoring"
    target: "./k8s/monitoring"
    mustSucceed: false              # Optional: warns on failure, doesn't abort
```

### Pull a single file

```bash
tixgraft --repository my_org/configs \
  --pull-source docker/Dockerfile.node \
  --pull-target ./Dockerfile \
  --pull-type file \
  --pull-replacement "{{NODE_VERSION}}=20"
```

### Orchestrate multiple services with children

```yaml
# tixgraft.yaml (project root)
children:
  - "./services/api/tixgraft.yaml"
  - "./services/web/tixgraft.yaml"
  - "./services/worker/tixgraft.yaml"
# Each child config is independent with its own repository, tag, pulls, etc.
```

### Generate a shareable command from config

```bash
tixgraft --to-command-line
# Outputs the CLI form of your tixgraft.yaml, within the limits under Config-to-CLI Conversion
```

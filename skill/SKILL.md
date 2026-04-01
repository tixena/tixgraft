---
name: tixgraft
description: Guide for using tixgraft (also called "graft") to pull or graft reusable components from Git repositories via sparse checkout, YAML config, CLI arguments, text replacements, and context-driven templating.
user-invocable: true
---

# tixgraft (graft)

tixgraft is a CLI tool for fetching reusable components from Git repositories using sparse checkout. When users say "graft" they mean tixgraft — e.g., "graft a component from github", "I want to graft this into my project", "pull this scaffold using graft".

## When to Use

Use tixgraft when the user wants to:
- Pull specific files or directories from a remote Git repository into their project
- Scaffold or template new components from a shared repository
- Apply text replacements (placeholders) on pulled content
- Run post-processing commands after pulling files
- Reuse infrastructure configs, service templates, or any shareable code from Git

## CLI Reference

### Core Flags

```
tixgraft                              # Run with ./tixgraft.yaml config
tixgraft --config <path>              # Use a specific config file
tixgraft --repository <repo>          # Git repo (overrides config)
tixgraft --tag <ref>                  # Branch, tag, or commit (overrides config)
tixgraft --dry-run                    # Preview without executing
tixgraft --verbose / -v               # Debug logging
tixgraft --to-command-line            # Convert config to CLI command
tixgraft --to-config                  # Convert CLI args to YAML config
tixgraft --output-format <fmt>        # "shell" (default) or "json" for --to-command-line
```

### Per-Pull Flags (repeatable, index-aligned)

Each `--pull-*` flag at index N pairs with other `--pull-*` flags at the same index:

```
--pull-source <path>              # Source path in the Git repository (required)
--pull-target <path>              # Target path in local workspace (required)
--pull-type <type>                # "file" or "directory" (default: "directory")
--pull-repository <repo>          # Override repository for this pull
--pull-tag <ref>                  # Override tag for this pull
--pull-reset                      # rm -rf target before copying
--pull-commands <cmds>            # Post-copy commands (comma-separated)
--pull-replacement <SRC=TGT>      # Text replacement: "{{PLACEHOLDER}}=value" or "{{VAR}}=env:ENV_NAME"
```

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

## Repository URL Formats

tixgraft accepts three repository formats:

| Format | Example | Expands To |
|--------|---------|------------|
| Short | `my_org/repo` | `https://github.com/my_org/repo.git` |
| HTTPS | `https://github.com/my_org/repo.git` | (used as-is) |
| SSH | `git@github.com:my_org/repo.git` | (used as-is) |

Enterprise Git hosts work with full HTTPS/SSH URLs.

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

# Pull operations (required, minimum 1)
pulls:
  - source: "path/in/repo"         # Required
    target: "./local/path"          # Required
    type: "directory"               # Optional: "file" or "directory"
    repository: "other/repo"        # Optional: override global
    tag: "v1.0.0"                   # Optional: override global
    reset: true                     # Optional: delete target first
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

**Config hierarchy**: CLI arguments > per-pull config > global config.

## Text Replacements

Replacements find-and-replace text in all non-binary files after copying.

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

### Providing Context

Context values can come from three sources (in priority order):

1. **CLI**: `--context serviceName=my-api --context port=8080`
2. **Per-pull config**: `context:` block under a pull
3. **Global config**: `context:` block at root level

For complex values use `--context-json`:
```bash
--context-json 'services=[{"name":"api","port":8080}]'
```

### Type Coercion

String values are automatically coerced to the declared type:
- `"true"`, `"yes"`, `"1"` -> boolean `true`
- `"8080"` -> number `8080`

### Validation

- Missing required properties (no default) -> exit code 1
- Invalid types that can't be coerced -> exit code 1
- Extra properties not in context definition -> warning, continues

### Processing Flow

1. Files are copied to target
2. `.graft.yaml` files are discovered recursively in target
3. Context is validated against requirements
4. Replacements are applied using context values
5. Post-commands execute
6. `.graft.yaml` files are cleaned up (removed from target)

## Config-to-CLI Conversion

Convert any YAML config to a shareable CLI command:

```bash
tixgraft --to-command-line                    # Shell format
tixgraft --to-command-line --output-format json  # JSON array
tixgraft --to-command-line --repository override/repo  # With overrides
```

Convert CLI args to a YAML config:

```bash
tixgraft --to-config --repository my_org/repo --pull-source src --pull-target ./dest
```

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
| 0 | Success |
| 1 | Configuration error |
| 2 | Source error (path not found in repo) |
| 3 | Command error (post-processing command failed) |
| 4 | Git error (clone/checkout failed) |
| 5 | Filesystem error |
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
repository: "devops/k8s-scaffolds"
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
```

### Pull a single file

```bash
tixgraft --repository my_org/configs \
  --pull-source docker/Dockerfile.node \
  --pull-target ./Dockerfile \
  --pull-type file \
  --pull-replacement "{{NODE_VERSION}}=20"
```

### Generate a shareable command from config

```bash
tixgraft --to-command-line
# Outputs the full CLI equivalent of your tixgraft.yaml
```

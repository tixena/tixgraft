# AGENTS.md

This file provides guidance for AI agents working with code in this repository.

## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Auto-syncs to JSONL for version control
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**
```bash
bd ready --json                    # Show all unblocked work
bd ready --sort priority --json    # Sort by strict priority
bd ready --sort oldest --json      # Sort by age (backlog clearing)
bd ready --sort hybrid --json      # Default: blend priority + age
bd blocked --json                  # Show work blocked by dependencies
```

**Create new issues:**
```bash
# Basic creation
bd create "Issue title" -t bug|feature|task -p 0-4 --json

# With description and labels
bd create "Issue title" -d "Detailed description" -l backend,auth --json

# With dependencies
bd create "Issue title" -p 1 --deps discovered-from:bd-123 --json

# Batch creation from file
bd create -f issues.md --json
```

**Claim and update:**
```bash
bd update bd-42 --status in_progress --json
bd update bd-42 --priority 1 --json
bd update bd-42 --assignee username --json
```

**Complete work:**
```bash
bd close bd-42 --reason "Completed" --json
```

**Labels:**
```bash
bd label add bd-42 optimization --json
bd label remove bd-42 backend --json
bd list --label backend,auth --json        # AND logic (requires ALL)
bd list --label-any frontend,ui --json     # OR logic (matches ANY)
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Dependencies

bd supports **4 dependency types** for organizing work:

1. **blocks** (default) - Hard blockers preventing issue start
   - Only this type affects `bd ready` detection
   - Use for: Prerequisites that must be done first

2. **related** - Soft connections without blocking
   - Use for: Related work, similar issues, context

3. **parent-child** - Hierarchical relationships
   - Use for: Epics with subtasks, feature breakdowns

4. **discovered-from** - Work found during other tasks
   - Use for: Bugs/improvements found while implementing features

**Commands:**
```bash
# Add dependency (defaults to "blocks")
bd dep add bd-43 bd-42 --json                    # bd-42 blocks bd-43
bd dep add bd-43 bd-42 --type blocks --json      # Explicit type
bd dep add bd-43 bd-42 --type related --json     # Non-blocking link

# Remove dependency
bd dep remove bd-43 bd-42 --json

# Visualize dependency graph
bd dep tree bd-42 --json

# Detect circular dependencies
bd dep cycles --json
```

### Labels

Labels provide flexible metadata for organizing and filtering issues:

**Use cases:**
- Component tags: `backend`, `frontend`, `api`, `database`
- Feature areas: `auth`, `payments`, `notifications`
- Work types: `refactor`, `optimization`, `documentation`
- Custom organization: `urgent`, `tech-debt`, `good-first-issue`

**Commands:**
```bash
# Add during creation
bd create "Issue" -l backend,auth,urgent --json

# Manage labels
bd label add bd-42 optimization --json
bd label remove bd-42 backend --json

# Filter by labels
bd list --label backend,auth --json          # AND: must have ALL labels
bd list --label-any frontend,ui --json       # OR: must have ANY label
bd ready --label backend --json              # Ready work for backend only
```

### Workflow for AI Agents

**Standard workflow:**
1. **Check ready work**: `bd ready --json` shows unblocked issues
   - Use `--sort priority` for P0→P1→P2→P3 order
   - Use `--sort oldest` to clear backlog by age
   - Use `--label backend` to filter by component
2. **Understand dependencies**: `bd dep tree <id> --json` to visualize blockers
3. **Claim your task**: `bd update <id> --status in_progress --json`
4. **Work on it**: Implement, test, document
5. **Discover new work?** Create linked issue:
   - `bd create "Found bug" -p 1 -l <relevant-labels> --deps discovered-from:<parent-id> --json`
6. **Complete**: `bd close <id> --reason "Done" --json`

**When blocked:**
- Check blockers: `bd show <id> --json` to see what's blocking this issue
- Create blocker: `bd create "Blocker title" -p 1 --json`, then link it
- Mark dependencies: `bd dep add <blocked-id> <blocker-id> --json`
- Switch tasks: `bd ready --json` to find other unblocked work

**Working with epics:**
1. Create epic: `bd create "Feature name" -t epic -p 1 --json`
2. Break down tasks: `bd create "Subtask" -t task -p 1 --json` for each piece
3. Link children: `bd dep add <child-id> <epic-id> --type parent-child --json`
4. Track progress: `bd dep tree <epic-id> --json` to visualize all subtasks

### Advanced Features

**Statistics and monitoring:**
```bash
bd stats --json              # Project overview and statistics
bd info --json               # Database path and daemon status
bd list --json               # All issues with filtering
```

**Configuration management:**
```bash
bd config set jira.url "https://..." --json    # Store integration settings
bd config get jira.url --json                  # Retrieve settings
bd config list --json                          # List all config
bd config unset jira.url --json                # Remove settings
```

**Deletion (use with caution):**
```bash
bd delete bd-42 --json                    # Preview deletion (dry-run)
bd delete bd-42 --force --json            # Actually delete
bd delete bd-42 --cascade --force --json  # Recursively delete dependents
bd delete bd-1 bd-2 bd-3 --force --json   # Batch deletion
```

**Memory compression (for long-running projects):**
```bash
bd compact --dry-run --all --json         # Preview compression candidates
bd compact --days 90 --json               # AI-compress closed issues >90 days old
```

**Manual sync:**
```bash
bd sync --json    # Force immediate bidirectional sync (bypasses 5s debounce)
```

**Batch operations:**
```bash
bd create -f issues.md --json             # Batch create from markdown file
bd delete bd-1 bd-2 bd-3 --force --json   # Batch delete
# Performance: 1000 issues in ~950ms
```

### Auto-Sync

bd automatically syncs with git:
- Exports to `.beads/issues.jsonl` after changes (5s debounce)
- Imports from JSONL when newer (e.g., after `git pull`)
- Optional git hooks for zero-lag sync (install via `examples/git-hooks/install.sh`):
  - **pre-commit**: Immediate flush before committing
  - **post-merge**: Guaranteed import after `git pull`
- Manual sync: `bd sync --json` (bypasses debounce)
- No manual export/import needed for normal workflow!

### MCP Server (Recommended)

If using Claude or MCP-compatible clients, install the beads MCP server:

```bash
pip install beads-mcp
```

Add to MCP config (e.g., `~/.config/claude/config.json`):
```json
{
  "beads": {
    "command": "beads-mcp",
    "args": []
  }
}
```

Then use `mcp__beads__*` functions instead of CLI commands.

### Important Rules

**Always:**
- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Check `bd ready --json` before asking "what should I work on?"
- ✅ Use labels to organize work by component/area
- ✅ Link discovered work with `--deps discovered-from:<parent-id>`
- ✅ Visualize complex work with `bd dep tree <id> --json`
- ✅ Create epics for large features, break down into subtasks
- ✅ Use `bd blocked --json` to find what's waiting on dependencies
- ✅ Add descriptive labels during issue creation (`-l backend,auth`)
- ✅ Check for circular dependencies with `bd dep cycles --json`

**Never:**
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use TodoWrite tool (per CLAUDE.md)
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems
- ❌ Do NOT forget to close issues with `bd close <id> --reason "Done" --json`

**Best Practices:**
- Use **blocks** dependencies for hard prerequisites
- Use **related** for context/similar issues
- Use **parent-child** for epic→task relationships
- Use **discovered-from** for bugs found during implementation
- Sort ready work strategically (`--sort priority` for urgency, `--sort oldest` for backlog)
- Filter by labels to focus on specific areas (`bd ready --label backend --json`)
- Check dependency trees before starting complex work
- Use `bd stats --json` periodically to monitor project health

## Testing Patterns for AI Agents

This project uses the **System abstraction** to enable fast, isolated unit tests. Follow these patterns:

### Unit Tests - MUST Use MockSystem

**Critical Rules:**
- ✅ **ALWAYS** use `MockSystem::new()` for unit tests
- ✅ Use `system.create_temp_dir()` for temporary directories
- ✅ Set up test data with `.with_file()` and `.with_dir()`
- ❌ **NEVER** instantiate `RealSystem` in unit tests
- ❌ **NEVER** use `tempfile::TempDir` directly
- ❌ **NEVER** use `std::fs` operations directly in unit tests

**Example:**
```rust
use tixgraft::system::{MockSystem, System};

#[test]
fn test_feature() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/input.txt", b"data");

    let temp_dir = system.create_temp_dir().unwrap();
    // temp_dir is an in-memory directory that auto-cleans on drop

    // Test your feature...
}
```

### Integration Tests - RealSystem When Needed

**Use RealSystem only for:**
- Git operations (sparse checkout)
- Shell command execution
- End-to-end CLI testing

**Still prefer System abstraction:**
```rust
use tixgraft::system::{RealSystem, System};

#[test]
fn test_git_operation() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    // Real filesystem, automatic cleanup
}
```

### Why This Matters

- **Speed**: MockSystem tests are ~100x faster (no disk I/O)
- **Isolation**: No temp directory conflicts or cleanup issues
- **Determinism**: Consistent in-memory state every run
- **Parallelism**: Tests run concurrently without conflicts

See **CLAUDE.md Testing Guidelines** for complete documentation.

For more details on bd, see https://github.com/steveyegge/beads

//! Pull operation coordination.

use crate::cli::{Args, PullArgs, PullConfig, ReplacementConfig};
use crate::config::Config;
use crate::config::context::{ContextValues, ValidatedContext, merge_context_values};
use crate::config::graft_yaml::GraftConfig;
use crate::error::GraftError;
use crate::git::{Repository, SparseCheckout, check_git_availability};
use crate::operations::discovery::{DiscoveredGraft, cleanup_graft_files, discover_graft_files};
use crate::operations::post_commands::execute_post_commands;
use crate::operations::{
    apply_graft_replacements, apply_replacements, copy_files, execute_commands,
};
use crate::system::System;
use anyhow::{Context as _, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Max nesting depth for children configs.
const MAX_CHILDREN_DEPTH: usize = 11;

/// Coordinates the complete pull operation.
#[non_exhaustive]
#[expect(
    clippy::module_name_repetitions,
    reason = "PullOperation is the canonical name for this type and removing the prefix would reduce clarity"
)]
pub struct PullOperation<'src> {
    /// The merged configuration driving this pull operation.
    config: Config,
    /// The path to the config file that was loaded.
    config_path: String,
    /// Whether to only preview operations without executing them.
    dry_run: bool,
    /// The system abstraction for filesystem operations.
    system: &'src dyn System,
}

impl<'src> PullOperation<'src> {
    /// Execute the pull operation, including recursive child execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pull operation cannot be executed.
    /// - A child config fails to load, validate, or execute.
    /// - A circular dependency is detected among child configs.
    /// - Max nesting depth is exceeded.
    #[inline]
    pub fn execute(&self) -> Result<()> {
        if self.dry_run {
            return self.preview_operations();
        }

        let config_dir = Path::new(&self.config_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let mut visited = HashSet::new();

        execute_config_recursive(self.system, &self.config, config_dir, &mut visited, 0)
    }

    /// Quick check if a URL is a local filesystem path.
    fn is_local_url(url: &str) -> bool {
        url.starts_with("file://")
            || url.starts_with('~')
            || url.starts_with("./")
            || url.starts_with("../")
            || (url.starts_with('/') && !url.starts_with("git@") && !url.starts_with("http"))
    }

    /// Create a new pull operation from CLI arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration file cannot be loaded or parsed.
    /// - The configuration is invalid.
    /// - The configuration cannot be merged with CLI overrides.
    /// - Git availability check fails.
    #[inline]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Public API consumed by callers that pass Args by value; changing to &Args would break existing call sites"
    )]
    pub fn new(args: Args, system: &'src dyn System) -> Result<Self> {
        // Load configuration
        let mut config = if Path::new(&args.config).exists() {
            Config::load_from_file(system, &args.config)?
        } else if !args.config.ends_with("tixgraft.yaml") || !args.pulls.sources.is_empty() {
            // If non-default config file specified but doesn't exist, or CLI args provided, that's an error
            if !args.config.ends_with("tixgraft.yaml") {
                return Err(GraftError::configuration(format!(
                    "Configuration file not found: {}",
                    args.config
                ))
                .into());
            }

            // Create minimal config from CLI args
            Config {
                repository: args.repository.clone(),
                tag: args.tag.clone(),
                context: HashMap::new(),
                pulls: Vec::new(),
                children: Vec::new(),
                process_children_first: false,
            }
        } else {
            return Err(GraftError::configuration(
                "No configuration found. Create a tixgraft.yaml file or provide pull arguments via CLI".to_owned()
            ).into());
        };

        // Merge CLI arguments into config
        merge_cli_args(&mut config, &args)?;

        // Validate merged configuration
        config.validate(system)?;

        // Check if any pull operations require Git (i.e., not all are local)
        let needs_git = Self::requires_git(&config);

        if needs_git {
            // Only check Git availability if we have at least one Git-based pull
            check_git_availability().context("Git validation failed")?;
        }

        Ok(PullOperation {
            config,
            config_path: args.config.clone(),
            dry_run: args.dry_run,
            system,
        })
    }

    /// Preview operations without executing them.
    fn preview_operations(&self) -> Result<()> {
        info!("Dry run preview - no files will be modified:");
        info!("");
        info!("Planned operations:");

        let config_dir = Path::new(&self.config_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let mut visited = HashSet::new();

        preview_config_recursive(self.system, &self.config, config_dir, &mut visited, 0, "")?;

        info!("");
        info!("Run without --dry-run to execute these operations.");

        Ok(())
    }

    /// Check if the configuration requires Git (has at least one non-local repository).
    fn requires_git(config: &Config) -> bool {
        // Check global repository
        if let Some(repo_url) = config.repository.as_ref()
            && !Self::is_local_url(repo_url)
        {
            return true;
        }

        // Check per-pull repositories
        for pull in &config.pulls {
            if let Some(repo_url) = pull.repository.as_ref() {
                if !Self::is_local_url(repo_url) {
                    return true;
                }
            } else if config.repository.is_none() {
                // If pull has no repository and global has no repository, we'll error anyway
                // But be conservative and assume Git is needed
                return true;
            } else {
                debug!("Skipping pull: {}", pull.source);
            }
        }

        false
    }
}

/// Result of processing .graft.yaml files.
#[derive(Debug, Default)]
struct GraftProcessingResult {
    /// Number of post-commands executed across all graft files.
    commands_executed: usize,
    /// Number of text replacements applied across all graft files.
    replacements_applied: usize,
}

/// Result of a single pull operation.
#[derive(Debug)]
struct PullResult {
    /// Number of post-processing commands executed.
    commands_executed: usize,
    /// Number of files copied from source to target.
    files_copied: usize,
    /// Number of text replacements applied in copied files.
    replacements_applied: usize,
}

/// Execute a config recursively, processing pulls and children.
fn execute_config_recursive(
    system: &dyn System,
    config: &Config,
    config_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<()> {
    if depth > MAX_CHILDREN_DEPTH {
        return Err(GraftError::configuration(format!(
            "Max children depth ({MAX_CHILDREN_DEPTH}) exceeded"
        ))
        .into());
    }

    // Circular dependency check using canonical path
    let canonical = fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Err(GraftError::configuration(format!(
            "Circular dependency detected at: {}",
            config_dir.display()
        ))
        .into());
    }

    if config.process_children_first {
        execute_children(system, config, config_dir, visited, depth)?;
        execute_pulls(system, config)?;
    } else {
        execute_pulls(system, config)?;
        execute_children(system, config, config_dir, visited, depth)?;
    }

    // Remove from visited after processing to allow diamond-pattern
    // re-execution (same child referenced from multiple parents).
    // This is intentional: the circular check catches A -> B -> A cycles
    // within a single recursion stack, while allowing A -> B and A -> C
    // where both B and C reference D.
    visited.remove(&canonical);

    Ok(())
}

/// Execute all pull operations for a config.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Simple counter increments on usize totals that cannot realistically overflow"
)]
fn execute_pulls(system: &dyn System, config: &Config) -> Result<()> {
    if config.pulls.is_empty() {
        return Ok(());
    }

    info!("Starting tixgraft pull operation...");

    let mut total_files = 0_usize;
    let mut total_replacements = 0_usize;
    let mut total_commands = 0_usize;

    for (index, pull) in config.pulls.iter().enumerate() {
        let display_index = index.saturating_add(1);
        info!("\n=> Pull operation #{}", display_index);

        // Determine repository and reference
        let repo_url = pull
            .repository
            .as_ref()
            .or(config.repository.as_ref())
            .ok_or_else(|| {
                GraftError::configuration(format!(
                    "No repository specified for pull #{display_index}"
                ))
            })?;
        debug!("Repository URL: {}", repo_url);

        let default_tag = "main".to_owned();
        let reference = pull
            .tag
            .as_ref()
            .or(config.tag.as_ref())
            .unwrap_or(&default_tag)
            .clone();

        debug!("Pull config: {:?}", pull);
        let result = execute_single_pull(system, config, pull, repo_url, &reference)?;

        debug!("execute_single_pull Result: {:?}", result);

        total_files += result.files_copied;
        total_replacements += result.replacements_applied;
        total_commands += result.commands_executed;

        info!(
            "  \u{2713} {} \u{2192} {} ({}, {} files)",
            pull.source, pull.target, pull.pull_type, result.files_copied
        );
    }

    info!("\n\u{2713} Completed pull operations successfully");
    info!("  Files copied: {}", total_files);
    info!("  Text replacements: {}", total_replacements);
    info!("  Commands executed: {}", total_commands);

    Ok(())
}

/// Execute all children for a config.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Depth increment cannot overflow for practical recursion depths"
)]
fn execute_children(
    system: &dyn System,
    config: &Config,
    config_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<()> {
    for child_path_str in &config.children {
        let child_config_path = config_dir.join(child_path_str);
        let child_dir = child_config_path.parent().unwrap_or_else(|| Path::new("."));

        info!("\n=> Processing child config: {}", child_path_str);

        let child_config_path_str = child_config_path.to_string_lossy();
        // load_from_file validates the config with children paths resolved
        // relative to the config file's directory (not CWD).
        let child_config = Config::load_from_file(system, &child_config_path_str)
            .with_context(|| format!("Error in child '{child_path_str}': failed to load config"))?;

        // Resolve child pull targets relative to child config directory
        let mut resolved_config = child_config;
        for pull in &mut resolved_config.pulls {
            let resolved_target = child_dir.join(&pull.target);
            pull.target = resolved_target.to_string_lossy().to_string();
        }

        execute_config_recursive(system, &resolved_config, child_dir, visited, depth + 1)
            .with_context(|| format!("Error in child '{child_path_str}'"))?;
    }

    Ok(())
}

/// Preview a config recursively, showing pulls and children with hierarchy.
fn preview_config_recursive(
    system: &dyn System,
    config: &Config,
    config_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
    indent: &str,
) -> Result<()> {
    if depth > MAX_CHILDREN_DEPTH {
        return Err(GraftError::configuration(format!(
            "Max children depth ({MAX_CHILDREN_DEPTH}) exceeded"
        ))
        .into());
    }

    // Circular dependency check using canonical path
    let canonical = fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Err(GraftError::configuration(format!(
            "Circular dependency detected at: {}",
            config_dir.display()
        ))
        .into());
    }

    if config.process_children_first {
        preview_children(system, config, config_dir, visited, depth, indent)?;
        preview_pulls(config, indent)?;
    } else {
        preview_pulls(config, indent)?;
        preview_children(system, config, config_dir, visited, depth, indent)?;
    }

    // Remove from visited after processing to allow diamond-pattern
    visited.remove(&canonical);

    Ok(())
}

/// Preview all pull operations for a config at a given indentation level.
fn preview_pulls(config: &Config, indent: &str) -> Result<()> {
    for (index, pull) in config.pulls.iter().enumerate() {
        let display_index = index.saturating_add(1);
        let repo_url = pull
            .repository
            .as_ref()
            .or(config.repository.as_ref())
            .ok_or_else(|| {
                GraftError::configuration(format!(
                    "No repository specified for pull #{display_index}"
                ))
            })?;

        let default_tag = "main".to_owned();
        let reference = pull
            .tag
            .as_ref()
            .or(config.tag.as_ref())
            .unwrap_or(&default_tag);

        info!(
            "{indent}  [{}] Pull {} \u{2192} {} ({})",
            display_index, pull.source, pull.target, pull.pull_type
        );
        info!("{indent}      - Repository: {}", repo_url);
        info!("{indent}      - Reference: {}", reference);

        if pull.reset {
            info!("{indent}      - Would reset target directory (reset: true)");
        }

        if !pull.replacements.is_empty() {
            info!(
                "{indent}      - Would apply {} text replacements",
                pull.replacements.len()
            );
        }

        if !pull.commands.is_empty() {
            info!(
                "{indent}      - Would execute {} commands:",
                pull.commands.len()
            );
            for cmd in &pull.commands {
                info!("{indent}        * {}", cmd);
            }
        }
    }

    Ok(())
}

/// Preview all children for a config, recursing into each child.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Depth increment cannot overflow for practical recursion depths"
)]
fn preview_children(
    system: &dyn System,
    config: &Config,
    config_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
    indent: &str,
) -> Result<()> {
    for child_path_str in &config.children {
        let child_config_path = config_dir.join(child_path_str);
        let child_dir = child_config_path.parent().unwrap_or_else(|| Path::new("."));

        info!("");
        info!("{indent}  Child: {}", child_path_str);

        let child_config_path_str = child_config_path.to_string_lossy();
        // load_from_file validates the config with children paths resolved
        // relative to the config file's directory (not CWD).
        let child_config = Config::load_from_file(system, &child_config_path_str)
            .with_context(|| format!("Error in child '{child_path_str}': failed to load config"))?;

        // Resolve child pull targets relative to child config directory
        let mut resolved_config = child_config;
        for pull in &mut resolved_config.pulls {
            let resolved_target = child_dir.join(&pull.target);
            pull.target = resolved_target.to_string_lossy().to_string();
        }

        let child_indent = format!("{indent}  ");
        preview_config_recursive(
            system,
            &resolved_config,
            child_dir,
            visited,
            depth + 1,
            &child_indent,
        )
        .with_context(|| format!("Error in child '{child_path_str}'"))?;
    }

    Ok(())
}

/// Execute a single pull operation.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Simple counter increments on usize totals that cannot realistically overflow"
)]
fn execute_single_pull(
    system: &dyn System,
    config: &Config,
    pull: &PullConfig,
    repo_url: &str,
    reference: &str,
) -> Result<PullResult> {
    debug!("Executing single pull operation: {repo_url} - {reference:?}");

    // Create repository and determine source type
    let repository = Repository::new(system, repo_url).context("Failed to create repository")?;

    // For Git repositories, we need to keep the SparseCheckout alive to prevent
    // the TempDir from being deleted before we finish copying files
    let sparse_checkout_guard;

    // Get source path based on repository type
    let source_path = if repository.is_git() {
        debug!("Repository is a Git repository");

        // Git repository - use sparse checkout
        let sparse_checkout =
            SparseCheckout::new(repository, reference.to_owned(), pull.source.clone())
                .context("Failed to create sparse checkout")?;

        debug!("Sparse checkout created");

        // Execute sparse checkout
        let checkout_path = sparse_checkout
            .execute()
            .context("Sparse checkout failed")?;

        debug!("Sparse checkout executed");

        // Verify source exists
        if !sparse_checkout.source_exists() {
            let diagnostics = sparse_checkout.get_checkout_diagnostics()?;
            return Err(GraftError::from_source(format!(
                "Source path '{}' not found in repository '{}' at reference '{}'\n\n{}",
                pull.source, repo_url, reference, diagnostics
            ))
            .into());
        }

        // IMPORTANT: Keep sparse_checkout alive until after copy_files completes
        // to prevent TempDir cleanup
        sparse_checkout_guard = Some(sparse_checkout);

        checkout_path
    } else {
        debug!("Repository is a local filesystem");

        // Local filesystem - construct path directly
        let base_path = repository
            .local_path()
            .ok_or_else(|| GraftError::from_source("Invalid local repository".to_owned()))?;

        let source_path = base_path.join(&pull.source);

        // Verify source exists
        if !source_path.exists() {
            return Err(GraftError::from_source(format!(
                "Source path '{}' not found in local repository '{}'",
                pull.source, repo_url
            ))
            .into());
        }

        // Verify source matches expected type
        let is_file = source_path.is_file();
        let is_dir = source_path.is_dir();

        if pull.pull_type == "file" && !is_file {
            return Err(GraftError::from_source(format!(
                "Source path '{}' is not a file (type specified as 'file')",
                source_path.display()
            ))
            .into());
        }

        if pull.pull_type == "directory" && !is_dir {
            return Err(GraftError::from_source(format!(
                "Source path '{}' is not a directory (type specified as 'directory')",
                source_path.display()
            ))
            .into());
        }

        sparse_checkout_guard = None;

        source_path
    };

    // Copy files
    let files_copied = copy_files(
        system,
        &source_path,
        &pull.target,
        &pull.pull_type,
        pull.reset,
    )?;

    drop(sparse_checkout_guard);

    // Apply text replacements
    let mut replacements_applied = if pull.replacements.is_empty() {
        0
    } else {
        apply_replacements(system, &pull.target, &pull.replacements)
            .context("Text replacement failed")?
    };

    // Process .graft.yaml files (context feature)
    let graft_result = process_graft_files(system, config, pull)?;
    replacements_applied += graft_result.replacements_applied;

    // Execute commands
    let mut commands_executed = if pull.commands.is_empty() {
        0
    } else {
        // For file operations, commands should run in the parent directory
        let command_working_dir = if pull.pull_type == "file" {
            Path::new(&pull.target)
                .parent()
                .and_then(|parent| parent.to_str())
                .unwrap_or(&pull.target)
        } else {
            &pull.target
        };

        execute_commands(&pull.commands, command_working_dir).context("Command execution failed")?
    };
    commands_executed += graft_result.commands_executed;

    Ok(PullResult {
        commands_executed,
        files_copied,
        replacements_applied,
    })
}

/// Build context for a specific graft (with parent inheritance).
fn build_graft_context(
    _discovered: &DiscoveredGraft,
    base_context: &ContextValues,
) -> ContextValues {
    // For now, just use base context
    // A more sophisticated approach would cache parsed .graft.yaml files
    // and inherit context from parent grafts
    base_context.clone()
}

/// Process all .graft.yaml files in the target directory.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Simple counter increments on usize totals that cannot realistically overflow"
)]
fn process_graft_files(
    system: &dyn System,
    config: &Config,
    pull: &PullConfig,
) -> Result<GraftProcessingResult> {
    let target_path = Path::new(&pull.target);

    // Check if target exists and is a directory
    if !system.exists(target_path)? || !system.is_dir(target_path)? {
        // Target doesn't exist or isn't a directory, no graft files to process
        return Ok(GraftProcessingResult::default());
    }

    // Discover all .graft.yaml files
    let discovered_grafts = discover_graft_files(system, target_path)
        .context("Failed to discover .graft.yaml files")?;

    if discovered_grafts.is_empty() {
        // No .graft.yaml files found, nothing to do
        return Ok(GraftProcessingResult::default());
    }

    info!("  Found {} .graft.yaml file(s)", discovered_grafts.len());

    let mut total_replacements = 0_usize;
    let mut total_commands = 0_usize;

    // Merge root and pull-level context
    let base_context = merge_context_values(config.context.clone(), pull.context.clone());

    // Process each .graft.yaml in order (root first, then children)
    for discovered in &discovered_grafts {
        debug!("Processing .graft.yaml at: {}", discovered.path.display());

        // Load and parse .graft.yaml
        let graft_config =
            GraftConfig::load_from_file(system, &discovered.path).with_context(|| {
                format!(
                    "Failed to load .graft.yaml from: {}",
                    discovered.path.display()
                )
            })?;

        // Build context for this graft (inherit from parent)
        let graft_context = build_graft_context(discovered, &base_context);

        // Validate context requirements
        if graft_config.context.is_empty() {
            // No context defined, apply replacements without validation
            if !graft_config.replacements.is_empty() {
                let replacements = apply_graft_replacements(
                    system,
                    discovered.directory.to_str().ok_or_else(|| {
                        GraftError::filesystem("Invalid directory path".to_owned())
                    })?,
                    &graft_config.replacements,
                    &graft_context,
                )
                .context("Failed to apply graft replacements")?;

                total_replacements += replacements;
            }
        } else {
            let validated =
                ValidatedContext::new(graft_config.context.clone(), graft_context.clone())
                    .context("Context validation failed")?;

            debug!(
                "Validated context for .graft.yaml at: {}",
                discovered.directory.display()
            );

            // Apply graft replacements
            if !graft_config.replacements.is_empty() {
                let replacements = apply_graft_replacements(
                    system,
                    discovered.directory.to_str().ok_or_else(|| {
                        GraftError::filesystem("Invalid directory path".to_owned())
                    })?,
                    &graft_config.replacements,
                    &validated.values,
                )
                .context("Failed to apply graft replacements")?;

                total_replacements += replacements;
                debug!(
                    "Applied {} replacements in {}",
                    replacements,
                    discovered.directory.display()
                );
            }
        }

        // Execute post-commands
        if !graft_config.post_commands.is_empty() {
            let results = execute_post_commands(&graft_config.post_commands, &discovered.directory)
                .context("Failed to execute post-commands")?;

            total_commands += results.len();
            debug!(
                "Executed {} post-command(s) in {}",
                results.len(),
                discovered.directory.display()
            );

            // Log any command failures (but don't fail the operation)
            for result in results {
                if !result.success {
                    warn!(
                        "Post-command failed in {}: {}",
                        discovered.directory.display(),
                        result.error.unwrap_or_else(|| "Unknown error".to_owned())
                    );
                }
            }
        }
    }

    // Cleanup: Delete all .graft.yaml files
    let deleted =
        cleanup_graft_files(system, target_path).context("Failed to cleanup .graft.yaml files")?;

    debug!("Deleted {} .graft.yaml file(s)", deleted);

    Ok(GraftProcessingResult {
        commands_executed: total_commands,
        replacements_applied: total_replacements,
    })
}

/// Merge CLI arguments into configuration.
fn merge_cli_args(config: &mut Config, args: &Args) -> Result<()> {
    // Override global repository and tag if provided via CLI
    if let Some(repo) = args.repository.as_ref() {
        config.repository = Some(repo.clone());
    }

    if let Some(tag) = args.tag.as_ref() {
        config.tag = Some(tag.clone());
    }

    // Merge context from CLI arguments
    if !args.context.is_empty() || !args.context_json.is_empty() {
        let cli_context = args.parse_context()?;
        // Merge CLI context into config context (CLI takes precedence)
        config.context.extend(cli_context);
    }

    // If CLI pulls are provided, use them instead of config pulls
    if !args.pulls.sources.is_empty() {
        config.pulls = create_pulls_from_cli(&args.pulls)?;
    }

    Ok(())
}

/// Create pull configurations from CLI arguments.
fn create_pulls_from_cli(pull_args: &PullArgs) -> Result<Vec<PullConfig>> {
    let mut pulls = Vec::new();

    let count = pull_args.sources.len();
    if count == 0 {
        return Ok(pulls);
    }

    // Verify that targets match sources count
    if pull_args.targets.len() != count {
        return Err(GraftError::configuration(format!(
            "Mismatch: {} sources specified but {} targets. Each source must have a corresponding target",
            count, pull_args.targets.len()
        )).into());
    }

    for (source, target) in pull_args.sources.iter().zip(pull_args.targets.iter()) {
        let idx = pulls.len();
        let pull = PullConfig {
            source: source.clone(),
            target: target.clone(),
            pull_type: pull_args
                .types
                .get(idx)
                .cloned()
                .unwrap_or_else(|| "directory".to_owned()),
            repository: pull_args.repositories.get(idx).cloned(),
            tag: pull_args.tags.get(idx).cloned(),
            reset: pull_args.resets.get(idx).copied().unwrap_or(false),
            commands: pull_args
                .commands
                .get(idx)
                .map_or_else(Vec::new, |cmd_str| {
                    cmd_str
                        .split(',')
                        .map(|segment| segment.trim().to_owned())
                        .collect()
                }),
            replacements: parse_replacements_for_pull(pull_args, idx)?,
            context: HashMap::new(),
        };

        pulls.push(pull);
    }

    Ok(pulls)
}

/// Parse replacements for a specific pull index.
/// Replacements are matched to pulls based on occurrence order.
fn parse_replacements_for_pull(
    pull_args: &PullArgs,
    _pull_index: usize,
) -> Result<Vec<ReplacementConfig>> {
    let mut replacements = Vec::new();

    // Strategy: Each replacement is associated with the pull at the same index
    // If we have more replacements than pulls, extras are ignored
    // If we have fewer replacements, later pulls get no replacements

    // For now, we'll use a simple strategy: all replacements in the Vec
    // are distributed across pulls in the order they appear
    // This is a limitation of the current CLI structure

    // Better approach: group replacements by tracking which source they follow
    // For MVP, let's assume all replacements apply to all pulls (not ideal but simple)
    // TODO: Implement proper grouping based on argument order

    // For now, since we can't easily track argument order in clap,
    // we'll apply ALL replacements to ALL pulls when using CLI
    // This is a known limitation that should be documented

    for repl_str in &pull_args.replacements {
        let replacement = parse_replacement_string(repl_str)?;
        replacements.push(replacement);
    }

    Ok(replacements)
}

/// Parse a single replacement string in format "SOURCE=TARGET" or "SOURCE=env:VAR".
fn parse_replacement_string(input: &str) -> Result<ReplacementConfig> {
    let (source_str, target_part) = input.split_once('=').ok_or_else(|| {
        GraftError::configuration(format!(
            "Invalid replacement format: '{input}'. Expected 'SOURCE=TARGET' or 'SOURCE=env:VAR'"
        ))
    })?;

    let source = source_str.to_owned();

    if let Some(env_var) = target_part.strip_prefix("env:") {
        Ok(ReplacementConfig {
            source,
            target: None,
            value_from_env: Some(env_var.to_owned()),
        })
    } else {
        Ok(ReplacementConfig {
            source,
            target: Some(target_part.to_owned()),
            value_from_env: None,
        })
    }
}

/// Build a Config structure from CLI arguments only (no file loading).
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be loaded or parsed.
/// - The configuration is invalid.
/// - The configuration cannot be merged with CLI overrides.
#[inline]
pub fn build_config_from_args(args: &Args) -> Result<Config> {
    let mut config = Config {
        repository: args.repository.clone(),
        tag: args.tag.clone(),
        context: HashMap::new(),
        pulls: Vec::new(),
        children: Vec::new(),
        process_children_first: false,
    };

    // Convert CLI pulls to config pulls
    if !args.pulls.sources.is_empty() {
        config.pulls = create_pulls_from_cli(&args.pulls)?;
    }

    if config.pulls.is_empty() && config.children.is_empty() {
        return Err(GraftError::configuration(
            "No pull operations specified. Use --pull-source and --pull-target to define at least one pull.".to_owned()
        ).into());
    }

    Ok(config)
}

/// Build a Config structure from both config file and CLI overrides.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be loaded or parsed.
/// - The configuration is invalid.
/// - The configuration cannot be merged with CLI overrides.
#[inline]
pub fn build_merged_config(args: &Args, system: &dyn System) -> Result<Config> {
    // Load base config if exists
    let mut config = if Path::new(&args.config).exists() {
        Config::load_from_file(system, &args.config)?
    } else {
        Config {
            repository: None,
            tag: None,
            context: HashMap::new(),
            pulls: Vec::new(),
            children: Vec::new(),
            process_children_first: false,
        }
    };

    // Apply CLI overrides
    merge_cli_args(&mut config, args)?;

    if config.pulls.is_empty() && config.children.is_empty() {
        return Err(GraftError::configuration(
            "No pull operations defined. Specify pulls in config file or via --pull-source/--pull-target.".to_owned()
        ).into());
    }

    Ok(config)
}

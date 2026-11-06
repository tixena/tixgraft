//! Runs a configuration's pulls and child configs, or previews them on a dry run.
//!
//! A pull fetches its source, copies it, applies replacements, processes the `.graft.yaml` files it
//! brought and runs its commands.

use crate::cli::{Args, PullArgs, PullConfig, ReplacementConfig};
use crate::config::Config;
use crate::config::context::{ContextValues, ValidatedContext, merge_context_values};
use crate::config::graft_yaml::GraftConfig;
use crate::config::validation::validate_config_with_base_dir;
use crate::error::GraftError;
use crate::git::{Repository, SparseCheckout, check_git_support, is_local_source};
use crate::operations::discovery::{DiscoveredGraft, cleanup_graft_files, discover_graft_files};
use crate::operations::post_commands::execute_post_commands;
use crate::operations::{
    apply_graft_replacements, apply_replacements, copy_files, execute_commands,
};
use anyhow::{Context as _, Result};
use git2::{Repository as GitRepository, Status, StatusOptions, StatusShow};
use os_shim::System;
use pollster::FutureExt as _;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Deepest nesting allowed for child configs, with the root config at depth 0.
const MAX_CHILDREN_DEPTH: usize = 11;

/// One invocation: the merged configuration, and whether it runs or only previews.
#[non_exhaustive]
#[expect(
    clippy::module_name_repetitions,
    reason = "PullOperation is the canonical name for this type and removing the prefix would reduce clarity"
)]
pub struct PullOperation<'src> {
    /// The configuration with the CLI arguments already merged over it.
    config: Config,
    /// The config file path, whose parent anchors relative targets and children.
    config_path: String,
    /// Whether to only preview operations without executing them.
    dry_run: bool,
    /// The filesystem and environment layer handed to each step.
    system: &'src dyn System,
}

impl<'src> PullOperation<'src> {
    /// Runs the pulls and child configs, or only previews them on a dry run.
    ///
    /// # Errors
    ///
    /// Returns an error if a required pull or a child fails, a cycle exists or nesting is too deep.
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

    /// Shares the detector `Repository::new` uses, so the Git check cannot disagree with it.
    fn is_local_url(url: &str) -> bool {
        is_local_source(url)
    }

    /// Loads the config file, merges the CLI arguments over it, validates the result and, when a
    /// pull needs Git, checks that Git is available.
    ///
    /// # Errors
    ///
    /// Returns an error if no configuration exists, it is invalid or a needed Git is unusable.
    #[inline]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Public API consumed by callers that pass Args by value; changing to &Args would break existing call sites"
    )]
    pub fn new(args: Args, system: &'src dyn System) -> Result<Self> {
        let mut config = if Path::new(&args.config).exists() {
            Config::load_from_file(system, &args.config)?
        } else if !args.config.ends_with("tixgraft.yaml") || !args.pulls.sources.is_empty() {
            if !args.config.ends_with("tixgraft.yaml") {
                return Err(GraftError::configuration(format!(
                    "Configuration file not found: {}",
                    args.config
                ))
                .into());
            }

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

        merge_cli_args(&mut config, &args)?;

        // Children paths resolve against the config file's directory, not the working directory.
        let base_dir = Path::new(&args.config).parent();
        validate_config_with_base_dir(system, &config, base_dir)?;

        let needs_git = Self::requires_git(&config);

        if needs_git {
            check_git_support().context("Git validation failed")?;
        }

        Ok(PullOperation {
            config,
            config_path: args.config.clone(),
            dry_run: args.dry_run,
            system,
        })
    }

    /// Logs the plan for the whole config tree, failing on a dirty target but changing nothing.
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

    /// True when any repository in use is not local, or when a pull has no repository to use.
    fn requires_git(config: &Config) -> bool {
        if let Some(repo_url) = config.repository.as_ref()
            && !Self::is_local_url(repo_url)
        {
            return true;
        }

        for pull in &config.pulls {
            if let Some(repo_url) = pull.repository.as_ref() {
                if !Self::is_local_url(repo_url) {
                    return true;
                }
            } else if config.repository.is_none() {
                return true;
            } else {
                debug!("Skipping pull: {}", pull.source);
            }
        }

        false
    }
}

/// Totals from processing the `.graft.yaml` files of one pull's target.
#[derive(Debug, Default)]
struct GraftProcessingResult {
    /// Post-commands run across all graft files, failed ones included.
    commands_executed: usize,
    /// Files changed across all graft files, counted once per replacement.
    replacements_applied: usize,
}

/// Totals from one pull, its graft files included.
#[derive(Debug)]
struct PullResult {
    /// Pull commands and `.graft.yaml` post-commands run.
    commands_executed: usize,
    /// Number of files copied from source to target.
    files_copied: usize,
    /// Files changed, counted once per replacement, `.graft.yaml` replacements included.
    replacements_applied: usize,
}

/// Joins relative targets onto `config_dir`, so a target means the same from any working directory.
fn resolve_pull_targets(config: &Config, config_dir: &Path) -> Config {
    let mut resolved = config.clone();
    for pull in &mut resolved.pulls {
        let target_path = Path::new(&pull.target);
        if !target_path.is_absolute() {
            let resolved_target = config_dir.join(target_path);
            pull.target = resolved_target.to_string_lossy().to_string();
        }
    }
    resolved
}

/// Runs a config's pulls and children in its configured order, rejecting cycles and excess depth.
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

    let canonical = fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Err(GraftError::configuration(format!(
            "Circular dependency detected at: {}",
            config_dir.display()
        ))
        .into());
    }

    let resolved_config = resolve_pull_targets(config, config_dir);

    if resolved_config.process_children_first {
        execute_children(system, &resolved_config, config_dir, visited, depth)?;
        execute_pulls(system, &resolved_config)?;
    } else {
        execute_pulls(system, &resolved_config)?;
        execute_children(system, &resolved_config, config_dir, visited, depth)?;
    }

    // A shared child may run again: only the recursion stack stays in the set, so only cycles fail.
    visited.remove(&canonical);

    Ok(())
}

/// Runs each pull in order and logs totals; a pull with `mustSucceed: false` only warns on failure.
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
        match execute_single_pull(system, config, pull, repo_url, &reference) {
            Ok(result) => {
                debug!("execute_single_pull Result: {:?}", result);

                total_files += result.files_copied;
                total_replacements += result.replacements_applied;
                total_commands += result.commands_executed;

                info!(
                    "  \u{2713} {} \u{2192} {} ({}, {} files)",
                    pull.source, pull.target, pull.pull_type, result.files_copied
                );
            }
            Err(err) => {
                if pull.must_succeed {
                    return Err(err);
                }
                warn!(
                    "  \u{26a0} Pull #{} failed (mustSucceed: false, continuing): {}",
                    display_index, err
                );
            }
        }
    }

    info!("\n\u{2713} Completed pull operations successfully");
    info!("  Files copied: {}", total_files);
    info!("  Text replacements: {}", total_replacements);
    info!("  Commands executed: {}", total_commands);

    Ok(())
}

/// Loads each child config, named relative to `config_dir`, and runs it one level deeper.
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
        let child_config = Config::load_from_file(system, &child_config_path_str)
            .with_context(|| format!("Error in child '{child_path_str}': failed to load config"))?;

        execute_config_recursive(system, &child_config, child_dir, visited, depth + 1)
            .with_context(|| format!("Error in child '{child_path_str}'"))?;
    }

    Ok(())
}

/// Logs a config's pulls and children in run order, rejecting cycles and excess depth.
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

    let canonical = fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Err(GraftError::configuration(format!(
            "Circular dependency detected at: {}",
            config_dir.display()
        ))
        .into());
    }

    let resolved_config = resolve_pull_targets(config, config_dir);

    if resolved_config.process_children_first {
        preview_children(system, &resolved_config, config_dir, visited, depth, indent)?;
        preview_pulls(&resolved_config, indent)?;
    } else {
        preview_pulls(&resolved_config, indent)?;
        preview_children(system, &resolved_config, config_dir, visited, depth, indent)?;
    }

    // A shared child may be previewed again: only the recursion stack stays in the set.
    visited.remove(&canonical);

    Ok(())
}

/// Logs each pull's plan at `indent`, failing on a dirty target that a run would also refuse.
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

        if pull.require_clean_target {
            check_target_is_clean(&pull.target)?;
        } else {
            info!("{indent}      - Skipping clean-target check (requireCleanTarget: false)");
        }

        if !pull.must_succeed {
            info!("{indent}      - Non-fatal: failures will be warnings (mustSucceed: false)");
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

/// Loads each child config, named relative to `config_dir`, and previews it one level deeper.
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
        let child_config = Config::load_from_file(system, &child_config_path_str)
            .with_context(|| format!("Error in child '{child_path_str}': failed to load config"))?;

        let child_indent = format!("{indent}  ");
        preview_config_recursive(
            system,
            &child_config,
            child_dir,
            visited,
            depth + 1,
            &child_indent,
        )
        .with_context(|| format!("Error in child '{child_path_str}'"))?;
    }

    Ok(())
}

/// Checks the target, fetches the source, validates graft context, copies, applies replacements,
/// processes graft files and runs the commands.
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

    if pull.require_clean_target {
        check_target_is_clean(&pull.target)?;
    }

    let repository = Repository::new(system, repo_url).context("Failed to create repository")?;

    let sparse_checkout_guard;

    let source_path = if repository.is_git() {
        debug!("Repository is a Git repository");

        let sparse_checkout =
            SparseCheckout::new(repository, reference.to_owned(), pull.source.clone())
                .context("Failed to create sparse checkout")?;

        debug!("Sparse checkout created");

        let checkout_path = sparse_checkout
            .execute()
            .context("Sparse checkout failed")?;

        debug!("Sparse checkout executed");

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

        let base_path = repository
            .local_path()
            .ok_or_else(|| GraftError::from_source("Invalid local repository".to_owned()))?;

        let source_path = base_path.join(&pull.source);

        if !source_path.exists() {
            return Err(GraftError::from_source(format!(
                "Source path '{}' not found in local repository '{}'",
                pull.source, repo_url
            ))
            .into());
        }

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

    // Validate .graft.yaml context from the source, before reset deletes the target.
    validate_graft_context(system, config, pull, &source_path)?;

    let files_copied = copy_files(
        system,
        &source_path,
        &pull.target,
        &pull.pull_type,
        pull.reset,
    )?;

    drop(sparse_checkout_guard);

    let mut replacements_applied = if pull.replacements.is_empty() {
        0
    } else {
        apply_replacements(system, &pull.target, &pull.replacements)
            .context("Text replacement failed")?
    };

    let graft_result = process_graft_files(system, config, pull)?;
    replacements_applied += graft_result.replacements_applied;

    let mut commands_executed = if pull.commands.is_empty() {
        0
    } else {
        // A file target has no directory of its own, so its commands run in its parent.
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

/// Returns the pull's merged context unchanged: a graft inherits nothing from its parent grafts.
fn build_graft_context(
    _discovered: &DiscoveredGraft,
    base_context: &ContextValues,
) -> ContextValues {
    base_context.clone()
}

/// Validates the context every `.graft.yaml` beneath `source_path` requires, before the target is
/// touched. It only reads, so a local source, the user's own directory, stays unchanged.
fn validate_graft_context(
    system: &dyn System,
    config: &Config,
    pull: &PullConfig,
    source_path: &Path,
) -> Result<()> {
    if !system.is_dir(source_path).block_on()? {
        return Ok(());
    }

    let discovered_grafts = discover_graft_files(system, source_path)
        .context("Failed to discover .graft.yaml files")?;

    let base_context = merge_context_values(config.context.clone(), pull.context.clone());

    for discovered in &discovered_grafts {
        let graft_config =
            GraftConfig::load_from_file(system, &discovered.path).with_context(|| {
                format!(
                    "Failed to load .graft.yaml from: {}",
                    discovered.path.display()
                )
            })?;

        if graft_config.context.is_empty() {
            continue;
        }

        let graft_context = build_graft_context(discovered, &base_context);
        ValidatedContext::new(graft_config.context, graft_context)
            .context("Context validation failed")?;
    }

    Ok(())
}

/// For each `.graft.yaml` under the target, root first, applies its replacements and runs its
/// post-commands, whose failures only warn; then deletes the files.
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

    if !system.exists(target_path).block_on()? || !system.is_dir(target_path).block_on()? {
        return Ok(GraftProcessingResult::default());
    }

    let discovered_grafts = discover_graft_files(system, target_path)
        .context("Failed to discover .graft.yaml files")?;

    if discovered_grafts.is_empty() {
        return Ok(GraftProcessingResult::default());
    }

    info!("  Found {} .graft.yaml file(s)", discovered_grafts.len());

    let mut total_replacements = 0_usize;
    let mut total_commands = 0_usize;

    let base_context = merge_context_values(config.context.clone(), pull.context.clone());

    for discovered in &discovered_grafts {
        debug!("Processing .graft.yaml at: {}", discovered.path.display());

        let graft_config =
            GraftConfig::load_from_file(system, &discovered.path).with_context(|| {
                format!(
                    "Failed to load .graft.yaml from: {}",
                    discovered.path.display()
                )
            })?;

        let graft_context = build_graft_context(discovered, &base_context);

        if graft_config.context.is_empty() {
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
            let validated = ValidatedContext::new(graft_config.context.clone(), graft_context)
                .context("Context validation failed")?;

            debug!(
                "Validated context for .graft.yaml at: {}",
                discovered.directory.display()
            );

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

        if !graft_config.post_commands.is_empty() {
            let results = execute_post_commands(&graft_config.post_commands, &discovered.directory)
                .context("Failed to execute post-commands")?;

            total_commands += results.len();
            debug!(
                "Executed {} post-command(s) in {}",
                results.len(),
                discovered.directory.display()
            );

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

    let deleted =
        cleanup_graft_files(system, target_path).context("Failed to cleanup .graft.yaml files")?;

    debug!("Deleted {} .graft.yaml file(s)", deleted);

    Ok(GraftProcessingResult {
        commands_executed: total_commands,
        replacements_applied: total_replacements,
    })
}

/// Overrides the configuration with the CLI's repository, tag and context; CLI pulls replace all
/// configured pulls.
fn merge_cli_args(config: &mut Config, args: &Args) -> Result<()> {
    if let Some(repo) = args.repository.as_ref() {
        config.repository = Some(repo.clone());
    }

    if let Some(tag) = args.tag.as_ref() {
        config.tag = Some(tag.clone());
    }

    if !args.context.is_empty() || !args.context_json.is_empty() {
        let cli_context = args.parse_context()?;
        config.context.extend(cli_context);
    }

    if !args.pulls.sources.is_empty() {
        config.pulls = create_pulls_from_cli(&args.pulls)?;
    }

    Ok(())
}

/// Builds one pull per `--pull-source`, paired by position with its target and per-pull flags.
fn create_pulls_from_cli(pull_args: &PullArgs) -> Result<Vec<PullConfig>> {
    let mut pulls = Vec::new();

    let count = pull_args.sources.len();
    if count == 0 {
        return Ok(pulls);
    }

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
            require_clean_target: pull_args
                .require_clean_targets
                .get(idx)
                .copied()
                .unwrap_or(true),
            must_succeed: pull_args.must_succeeds.get(idx).copied().unwrap_or(true),
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

/// Parses the replacements for one pull. Every pull receives all of them, since `PullArgs` records
/// no link between a replacement and the source it follows.
fn parse_replacements_for_pull(
    pull_args: &PullArgs,
    _pull_index: usize,
) -> Result<Vec<ReplacementConfig>> {
    let mut replacements = Vec::new();

    // TODO: Implement proper grouping based on argument order

    for repl_str in &pull_args.replacements {
        let replacement = parse_replacement_string(repl_str)?;
        replacements.push(replacement);
    }

    Ok(replacements)
}

/// Parses `SOURCE=TARGET` or `SOURCE=env:VAR`, splitting at the first `=`.
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

/// Builds a configuration from the CLI arguments alone, reading no file.
///
/// # Errors
///
/// Returns an error if the pull arguments are malformed or define no pull.
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

/// Builds a configuration from the config file, when it exists, with the CLI arguments over it.
///
/// # Errors
///
/// Returns an error if the file or the arguments are invalid or they define no pull or child.
#[inline]
pub fn build_merged_config(args: &Args, system: &dyn System) -> Result<Config> {
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

    merge_cli_args(&mut config, args)?;

    if config.pulls.is_empty() && config.children.is_empty() {
        return Err(GraftError::configuration(
            "No pull operations defined. Specify pulls in config file or via --pull-source/--pull-target.".to_owned()
        ).into());
    }

    Ok(config)
}

/// Fails when the target has uncommitted changes, listing up to ten of them. A target that is
/// missing, outside a working tree or unreadable by Git passes.
fn check_target_is_clean(target: &str) -> Result<()> {
    let target_path = Path::new(target);

    if !target_path.exists() {
        debug!(
            "Target path '{}' does not exist yet, skipping clean check",
            target
        );
        return Ok(());
    }

    // Starts at the target, so its repository is found even outside the working directory.
    let search_from = if target_path.is_dir() {
        target_path.to_path_buf()
    } else {
        target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    };

    let Ok(repository) = GitRepository::discover(&search_from) else {
        debug!(
            "Target '{}' is not inside a git repository, skipping clean check",
            target
        );
        return Ok(());
    };

    let Some(workdir) = repository.workdir() else {
        debug!(
            "Repository holding '{}' has no working tree, skipping clean check",
            target
        );
        return Ok(());
    };

    let Some(pathspec) = relative_pathspec(target_path, workdir) else {
        debug!(
            "Could not place target '{}' inside its repository, skipping clean check",
            target
        );
        return Ok(());
    };

    let mut options = StatusOptions::new();
    options
        .show(StatusShow::IndexAndWorkdir)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    if !pathspec.is_empty() {
        options.pathspec(&pathspec);
    }

    let Ok(statuses) = repository.statuses(Some(&mut options)) else {
        debug!(
            "Could not read git status for target '{}', skipping clean check",
            target
        );
        return Ok(());
    };

    let dirty_files: Vec<String> = statuses
        .iter()
        .filter_map(|entry| {
            entry
                .path()
                .ok()
                .map(|path| format!("{} {path}", status_code(entry.status())))
        })
        .collect();

    if dirty_files.is_empty() {
        return Ok(());
    }

    let max_shown: usize = 10;
    let file_list: String = dirty_files
        .iter()
        .take(max_shown)
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    let extra = if dirty_files.len() > max_shown {
        format!(
            "\n  ... and {} more",
            dirty_files.len().saturating_sub(max_shown)
        )
    } else {
        String::new()
    };

    Err(GraftError::filesystem(format!(
        "Target path '{target}' has uncommitted changes. \
         Commit or stash your changes before pulling, \
         or set requireCleanTarget: false to skip this check.\n\
         Dirty files:\n{file_list}{extra}"
    ))
    .into())
}

/// Express `target` relative to the repository working tree, for use as a status
/// pathspec. An empty string means the target is the working tree root.
fn relative_pathspec(target: &Path, workdir: &Path) -> Option<String> {
    let absolute_target = target.canonicalize().ok()?;
    let absolute_workdir = workdir.canonicalize().ok()?;
    let relative = absolute_target.strip_prefix(&absolute_workdir).ok()?;

    relative.to_str().map(ToOwned::to_owned)
}

/// Renders a status flag set as a short label in the style of `git status --porcelain`.
fn status_code(status: Status) -> &'static str {
    if status.is_wt_new() {
        "??"
    } else if status.is_index_new() {
        "A "
    } else if status.is_wt_deleted() || status.is_index_deleted() {
        " D"
    } else if status.is_wt_renamed() || status.is_index_renamed() {
        "R "
    } else if status.is_index_modified() {
        "M "
    } else {
        " M"
    }
}

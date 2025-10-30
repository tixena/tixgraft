//! Pull operation coordination

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
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Coordinates the complete pull operation
#[non_exhaustive]
#[expect(clippy::module_name_repetitions, reason = "PullOperation")]
pub struct PullOperation<'src> {
    config: Config,
    dry_run: bool,
    system: &'src dyn System,
}

impl<'src> PullOperation<'src> {
    /// Create a new pull operation from CLI arguments
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration file cannot be loaded or parsed
    /// - The configuration is invalid
    /// - The configuration cannot be merged with CLI overrides
    /// - Git availability check fails
    #[inline]
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
            dry_run: args.dry_run,
            system,
        })
    }

    /// Check if the configuration requires Git (has at least one non-local repository)
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

    /// Quick check if a URL is a local filesystem path
    fn is_local_url(url: &str) -> bool {
        url.starts_with("file://")
            || url.starts_with('~')
            || url.starts_with("./")
            || url.starts_with("../")
            || (url.starts_with('/') && !url.starts_with("git@") && !url.starts_with("http"))
    }

    /// Execute the pull operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pull operation cannot be executed
    #[inline]
    pub fn execute(&self) -> Result<()> {
        if self.dry_run {
            return self.preview_operations();
        }

        info!("Starting tixgraft pull operation...");

        let mut total_files = 0;
        let mut total_replacements = 0;
        let mut total_commands = 0;

        for (index, pull) in self.config.pulls.iter().enumerate() {
            info!("\n=> Pull operation #{}", index + 1);

            // Determine repository and reference
            let repo_url = pull
                .repository
                .as_ref()
                .or(self.config.repository.as_ref())
                .ok_or_else(|| {
                    GraftError::configuration(format!(
                        "No repository specified for pull #{}",
                        index + 1
                    ))
                })?;
            debug!("Repository URL: {}", repo_url);

            let default_tag = "main".to_owned();
            let reference = pull
                .tag
                .as_ref()
                .or(self.config.tag.as_ref())
                .unwrap_or(&default_tag)
                .clone();

            debug!("Pull config: {:?}", pull);
            // Execute single pull operation
            let result = self.execute_single_pull(pull, repo_url, &reference)?;

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

    /// Preview operations without executing them
    fn preview_operations(&self) -> Result<()> {
        info!("Dry run preview - no files will be modified:");
        info!("");
        info!("Planned operations:");

        for (index, pull) in self.config.pulls.iter().enumerate() {
            let repo_url = pull
                .repository
                .as_ref()
                .or(self.config.repository.as_ref())
                .ok_or_else(|| {
                    GraftError::configuration(format!(
                        "No repository specified for pull #{}",
                        index + 1
                    ))
                })?;

            let default_tag = "main".to_owned();
            let reference = pull
                .tag
                .as_ref()
                .or(self.config.tag.as_ref())
                .unwrap_or(&default_tag);

            info!(
                "  [{}] Pull {} \u{2192} {} ({})",
                index + 1,
                pull.source,
                pull.target,
                pull.pull_type
            );
            info!("      - Repository: {}", repo_url);
            info!("      - Reference: {}", reference);

            if pull.reset {
                info!("      - Would reset target directory (reset: true)");
            }

            if !pull.replacements.is_empty() {
                info!(
                    "      - Would apply {} text replacements",
                    pull.replacements.len()
                );
            }

            if !pull.commands.is_empty() {
                info!("      - Would execute {} commands:", pull.commands.len());
                for cmd in &pull.commands {
                    info!("        * {}", cmd);
                }
            }
        }

        info!("");
        info!("Run without --dry-run to execute these operations.");

        Ok(())
    }

    /// Execute a single pull operation
    fn execute_single_pull(
        &self,
        pull: &PullConfig,
        repo_url: &str,
        reference: &str,
    ) -> Result<PullResult> {
        debug!("Executing single pull operation: {repo_url} - {reference:?}");

        // Create repository and determine source type
        let repository =
            Repository::new(self.system, repo_url).context("Failed to create repository")?;

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
            self.system,
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
            apply_replacements(self.system, &pull.target, &pull.replacements)
                .context("Text replacement failed")?
        };

        // Process .graft.yaml files (context feature)
        let graft_result = self.process_graft_files(pull)?;
        replacements_applied += graft_result.replacements_applied;

        // Execute commands
        let mut commands_executed = if pull.commands.is_empty() {
            0
        } else {
            // For file operations, commands should run in the parent directory
            let command_working_dir = if pull.pull_type == "file" {
                Path::new(&pull.target)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or(&pull.target)
            } else {
                &pull.target
            };

            execute_commands(&pull.commands, command_working_dir)
                .context("Command execution failed")?
        };
        commands_executed += graft_result.commands_executed;

        Ok(PullResult {
            files_copied,
            replacements_applied,
            commands_executed,
        })
    }

    /// Process all .graft.yaml files in the target directory
    fn process_graft_files(&self, pull: &PullConfig) -> Result<GraftProcessingResult> {
        let target_path = Path::new(&pull.target);

        // Check if target exists and is a directory
        if !self.system.exists(target_path)? || !self.system.is_dir(target_path)? {
            // Target doesn't exist or isn't a directory, no graft files to process
            return Ok(GraftProcessingResult::default());
        }

        // Discover all .graft.yaml files
        let discovered_grafts = discover_graft_files(self.system, target_path)
            .context("Failed to discover .graft.yaml files")?;

        if discovered_grafts.is_empty() {
            // No .graft.yaml files found, nothing to do
            return Ok(GraftProcessingResult::default());
        }

        info!("  Found {} .graft.yaml file(s)", discovered_grafts.len());

        let mut total_replacements = 0;
        let mut total_commands = 0;

        // Merge root and pull-level context
        let base_context = merge_context_values(self.config.context.clone(), pull.context.clone());

        // Process each .graft.yaml in order (root first, then children)
        for discovered in &discovered_grafts {
            debug!("Processing .graft.yaml at: {}", discovered.path.display());

            // Load and parse .graft.yaml
            let graft_config = GraftConfig::load_from_file(self.system, &discovered.path)
                .with_context(|| {
                    format!(
                        "Failed to load .graft.yaml from: {}",
                        discovered.path.display()
                    )
                })?;

            // Build context for this graft (inherit from parent)
            let graft_context = Self::build_graft_context(discovered, &base_context);

            // Validate context requirements
            if graft_config.context.is_empty() {
                // No context defined, apply replacements without validation
                if !graft_config.replacements.is_empty() {
                    let replacements = apply_graft_replacements(
                        self.system,
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
                        self.system,
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
                let results =
                    execute_post_commands(&graft_config.post_commands, &discovered.directory)
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
        let deleted = cleanup_graft_files(self.system, target_path)
            .context("Failed to cleanup .graft.yaml files")?;

        debug!("Deleted {} .graft.yaml file(s)", deleted);

        Ok(GraftProcessingResult {
            replacements_applied: total_replacements,
            commands_executed: total_commands,
        })
    }

    /// Build context for a specific graft (with parent inheritance)
    fn build_graft_context(
        _discovered: &DiscoveredGraft,
        base_context: &ContextValues,
    ) -> ContextValues {
        // For now, just use base context
        // A more sophisticated approach would cache parsed .graft.yaml files
        // and inherit context from parent grafts
        base_context.clone()
    }
}

/// Result of processing .graft.yaml files
#[derive(Debug, Default)]
struct GraftProcessingResult {
    replacements_applied: usize,
    commands_executed: usize,
}

/// Result of a single pull operation
#[derive(Debug)]
struct PullResult {
    files_copied: usize,
    replacements_applied: usize,
    commands_executed: usize,
}

/// Merge CLI arguments into configuration
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

/// Create pull configurations from CLI arguments
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

    for i in 0..count {
        let pull = PullConfig {
            source: pull_args.sources[i].clone(),
            target: pull_args.targets[i].clone(),
            pull_type: pull_args
                .types
                .get(i)
                .cloned()
                .unwrap_or_else(|| "directory".to_owned()),
            repository: pull_args.repositories.get(i).cloned(),
            tag: pull_args.tags.get(i).cloned(),
            reset: pull_args.resets.get(i).copied().unwrap_or(false),
            commands: if let Some(cmd_str) = pull_args.commands.get(i) {
                cmd_str.split(',').map(|s| s.trim().to_owned()).collect()
            } else {
                Vec::new()
            },
            replacements: parse_replacements_for_pull(pull_args, i)?,
            context: HashMap::new(),
        };

        pulls.push(pull);
    }

    Ok(pulls)
}

/// Parse replacements for a specific pull index
/// Replacements are matched to pulls based on occurrence order
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

/// Parse a single replacement string in format "SOURCE=TARGET" or "SOURCE=env:VAR"
fn parse_replacement_string(input: &str) -> Result<ReplacementConfig> {
    let parts: Vec<&str> = input.splitn(2, '=').collect();

    if parts.len() != 2 {
        return Err(GraftError::configuration(format!(
            "Invalid replacement format: '{input}'. Expected 'SOURCE=TARGET' or 'SOURCE=env:VAR'"
        ))
        .into());
    }

    let source = parts[0].to_owned();
    let target_part = parts[1];

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

/// Build a Config structure from CLI arguments only (no file loading)
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be loaded or parsed
/// - The configuration is invalid
/// - The configuration cannot be merged with CLI overrides
#[inline]
pub fn build_config_from_args(args: &Args) -> Result<Config> {
    let mut config = Config {
        repository: args.repository.clone(),
        tag: args.tag.clone(),
        context: HashMap::new(),
        pulls: Vec::new(),
    };

    // Convert CLI pulls to config pulls
    if !args.pulls.sources.is_empty() {
        config.pulls = create_pulls_from_cli(&args.pulls)?;
    }

    if config.pulls.is_empty() {
        return Err(GraftError::configuration(
            "No pull operations specified. Use --pull-source and --pull-target to define at least one pull.".to_owned()
        ).into());
    }

    Ok(config)
}

/// Build a Config structure from both config file and CLI overrides
///
/// # Errors
///
/// Returns an error if:
/// - The configuration file cannot be loaded or parsed
/// - The configuration is invalid
/// - The configuration cannot be merged with CLI overrides
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
        }
    };

    // Apply CLI overrides
    merge_cli_args(&mut config, args)?;

    if config.pulls.is_empty() {
        return Err(GraftError::configuration(
            "No pull operations defined. Specify pulls in config file or via --pull-source/--pull-target.".to_owned()
        ).into());
    }

    Ok(config)
}

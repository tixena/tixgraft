//! Pull operation coordination

use crate::cli::{Args, PullArgs, PullConfig};
use crate::config::Config;
use crate::error::GraftError;
use crate::git::{Repository, SparseCheckout, check_git_availability};
use crate::operations::{apply_replacements, copy_files, execute_commands};
use crate::system::System;
use anyhow::{Context as _, Result};
use tracing::{debug, info};

/// Coordinates the complete pull operation
pub struct PullOperation<'a> {
    config: Config,
    dry_run: bool,
    system: &'a dyn System,
}

impl<'a> PullOperation<'a> {
    /// Create a new pull operation from CLI arguments
    pub fn new(args: Args, system: &'a dyn System) -> Result<Self> {
        // Load configuration
        let mut config = if std::path::Path::new(&args.config).exists() {
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
        if let Some(ref repo_url) = config.repository
            && !Self::is_local_url(repo_url)
        {
            return true;
        }

        // Check per-pull repositories
        for pull in &config.pulls {
            if let Some(ref repo_url) = pull.repository {
                if !Self::is_local_url(repo_url) {
                    return true;
                }
            } else if config.repository.is_none() {
                // If pull has no repository and global has no repository, we'll error anyway
                // But be conservative and assume Git is needed
                return true;
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
        let _sparse_checkout_guard;

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
                let diagnostics = sparse_checkout.get_checkout_diagnostics();
                return Err(GraftError::source(format!(
                    "Source path '{}' not found in repository '{}' at reference '{}'\n\n{}",
                    pull.source, repo_url, reference, diagnostics
                ))
                .into());
            }

            // IMPORTANT: Keep sparse_checkout alive until after copy_files completes
            // to prevent TempDir cleanup
            _sparse_checkout_guard = Some(sparse_checkout);

            checkout_path
        } else {
            debug!("Repository is a local filesystem");

            // Local filesystem - construct path directly
            let base_path = repository
                .local_path()
                .ok_or_else(|| return GraftError::source("Invalid local repository".to_owned()))?;

            let source_path = base_path.join(&pull.source);

            // Verify source exists
            if !source_path.exists() {
                return Err(GraftError::source(format!(
                    "Source path '{}' not found in local repository '{}'",
                    pull.source, repo_url
                ))
                .into());
            }

            // Verify source matches expected type
            let is_file = source_path.is_file();
            let is_dir = source_path.is_dir();

            if pull.pull_type == "file" && !is_file {
                return Err(GraftError::source(format!(
                    "Source path '{}' is not a file (type specified as 'file')",
                    source_path.display()
                ))
                .into());
            }

            if pull.pull_type == "directory" && !is_dir {
                return Err(GraftError::source(format!(
                    "Source path '{}' is not a directory (type specified as 'directory')",
                    source_path.display()
                ))
                .into());
            }

            _sparse_checkout_guard = None;

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

        // Apply text replacements
        let replacements_applied = if pull.replacements.is_empty() {
            0
        } else {
            apply_replacements(self.system, &pull.target, &pull.replacements)
                .context("Text replacement failed")?
        };

        // Execute commands
        let commands_executed = if !pull.commands.is_empty() {
            // For file operations, commands should run in the parent directory
            let command_working_dir = if pull.pull_type == "file" {
                std::path::Path::new(&pull.target)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or(&pull.target)
            } else {
                &pull.target
            };

            execute_commands(&pull.commands, command_working_dir)
                .context("Command execution failed")?
        } else {
            0
        };

        Ok(PullResult {
            files_copied,
            replacements_applied,
            commands_executed,
        })
    }
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
    if let Some(ref repo) = args.repository {
        config.repository = Some(repo.clone());
    }

    if let Some(ref tag) = args.tag {
        config.tag = Some(tag.clone());
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
                .unwrap_or_else(|| return "directory".to_owned()),
            repository: pull_args.repositories.get(i).cloned(),
            tag: pull_args.tags.get(i).cloned(),
            reset: pull_args.resets.get(i).copied().unwrap_or(false),
            commands: if let Some(cmd_str) = pull_args.commands.get(i) {
                cmd_str
                    .split(',')
                    .map(|s| return s.trim().to_owned())
                    .collect()
            } else {
                Vec::new()
            },
            replacements: Vec::new(), // CLI doesn't support replacements directly
        };

        pulls.push(pull);
    }

    Ok(pulls)
}

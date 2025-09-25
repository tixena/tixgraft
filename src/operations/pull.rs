//! Pull operation coordination

use anyhow::{Result, Context};
use colored::Colorize;
use crate::cli::{Args, PullConfig, PullArgs};
use crate::config::Config;
use crate::git::{Repository, SparseCheckout, check_git_availability};
use crate::error::GraftError;
use crate::operations::{copy_files, apply_replacements, execute_commands};

/// Coordinates the complete pull operation
pub struct PullOperation {
    config: Config,
    dry_run: bool,
}

impl PullOperation {
    /// Create a new pull operation from CLI arguments
    pub fn new(args: Args) -> Result<Self> {
        // First check Git availability
        check_git_availability()
            .context("Git validation failed")?;

        // Load configuration
        let mut config = if std::path::Path::new(&args.config).exists() {
            Config::load_from_file(&args.config)?
        } else if !args.config.ends_with("tixgraft.yaml") || !args.pulls.sources.is_empty() {
            // If non-default config file specified but doesn't exist, or CLI args provided, that's an error
            if !args.config.ends_with("tixgraft.yaml") {
                return Err(GraftError::configuration(format!(
                    "Configuration file not found: {}",
                    args.config
                )).into());
            }
            
            // Create minimal config from CLI args
            Config {
                repository: args.repository.clone(),
                tag: args.tag.clone(),
                pulls: Vec::new(),
            }
        } else {
            return Err(GraftError::configuration(
                "No configuration found. Create a tixgraft.yaml file or provide pull arguments via CLI".to_string()
            ).into());
        };

        // Merge CLI arguments into config
        merge_cli_args(&mut config, &args)?;

        // Validate merged configuration
        config.validate()?;
            // .context("Configuration validation failed")?;

        Ok(PullOperation {
            config,
            dry_run: args.dry_run,
        })
    }

    /// Execute the pull operation
    pub fn execute(&self) -> Result<()> {
        if self.dry_run {
            return self.preview_operations();
        }

        println!("{}", "Starting tixgraft pull operation...".bold().green());
        
        let mut total_files = 0;
        let mut total_replacements = 0;
        let mut total_commands = 0;

        for (index, pull) in self.config.pulls.iter().enumerate() {
            println!("\n{} Pull operation #{}", "=>".blue().bold(), index + 1);
            
            // Determine repository and reference
            let repo_url = pull.repository.as_ref()
                .or(self.config.repository.as_ref())
                .ok_or_else(|| GraftError::configuration(
                    format!("No repository specified for pull #{}", index + 1)
                ))?;
            
            let default_tag = "main".to_string();
            let reference = pull.tag.as_ref()
                .or(self.config.tag.as_ref())
                .unwrap_or(&default_tag)
                .clone();

            // Execute single pull operation
            let result = self.execute_single_pull(pull, repo_url, &reference)?;
            
            total_files += result.files_copied;
            total_replacements += result.replacements_applied;
            total_commands += result.commands_executed;

            println!("  {} {} → {} ({}, {} files)", 
                "✓".green().bold(),
                pull.source,
                pull.target,
                pull.pull_type,
                result.files_copied
            );
        }

        println!("\n{}", "✓ Completed pull operations successfully".bold().green());
        println!("  Files copied: {}", total_files);
        println!("  Text replacements: {}", total_replacements);
        println!("  Commands executed: {}", total_commands);

        Ok(())
    }

    /// Preview operations without executing them
    fn preview_operations(&self) -> Result<()> {
        println!("{}", "Dry run preview - no files will be modified:".bold().yellow());
        println!();
        println!("Planned operations:");

        for (index, pull) in self.config.pulls.iter().enumerate() {
            let repo_url = pull.repository.as_ref()
                .or(self.config.repository.as_ref())
                .ok_or_else(|| GraftError::configuration(
                    format!("No repository specified for pull #{}", index + 1)
                ))?;
            
            let default_tag = "main".to_string();
            let reference = pull.tag.as_ref()
                .or(self.config.tag.as_ref())
                .unwrap_or(&default_tag);

            println!("  [{}] Pull {} → {} ({})", 
                index + 1, pull.source, pull.target, pull.pull_type);
            println!("      - Repository: {}", repo_url);
            println!("      - Reference: {}", reference);
            
            if pull.reset {
                println!("      - Would reset target directory (reset: true)");
            }
            
            if !pull.replacements.is_empty() {
                println!("      - Would apply {} text replacements", pull.replacements.len());
            }
            
            if !pull.commands.is_empty() {
                println!("      - Would execute {} commands:", pull.commands.len());
                for cmd in &pull.commands {
                    println!("        * {}", cmd);
                }
            }
        }

        println!();
        println!("Run without --dry-run to execute these operations.");
        
        Ok(())
    }

    /// Execute a single pull operation
    fn execute_single_pull(&self, pull: &PullConfig, repo_url: &str, reference: &str) -> Result<PullResult> {
        // Create repository and sparse checkout
        let repository = Repository::new(repo_url)
            .context("Failed to create repository")?;
        
        let sparse_checkout = SparseCheckout::new(
            repository,
            reference.to_string(),
            pull.source.clone(),
        ).context("Failed to create sparse checkout")?;

        // Execute sparse checkout
        let source_path = sparse_checkout.execute()
            .context("Sparse checkout failed")?;

        // Verify source exists
        if !sparse_checkout.source_exists() {
            return Err(GraftError::source(format!(
                "Source path '{}' not found in repository '{}' at reference '{}'",
                pull.source, repo_url, reference
            )).into());
        }

        // Copy files
        let files_copied = copy_files(&source_path, &pull.target, &pull.pull_type, pull.reset)
            .context("File copying failed")?;

        // Apply text replacements
        let replacements_applied = if !pull.replacements.is_empty() {
            apply_replacements(&pull.target, &pull.replacements)
                .context("Text replacement failed")?
        } else {
            0
        };

        // Execute commands
        let commands_executed = if !pull.commands.is_empty() {
            execute_commands(&pull.commands, &pull.target)
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
            pull_type: pull_args.types.get(i).cloned().unwrap_or_else(|| "directory".to_string()),
            repository: pull_args.repositories.get(i).cloned(),
            tag: pull_args.tags.get(i).cloned(),
            reset: pull_args.resets.get(i).copied().unwrap_or(false),
            commands: if let Some(cmd_str) = pull_args.commands.get(i) {
                cmd_str.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                Vec::new()
            },
            replacements: Vec::new(), // CLI doesn't support replacements directly
        };
        
        pulls.push(pull);
    }

    Ok(pulls)
}

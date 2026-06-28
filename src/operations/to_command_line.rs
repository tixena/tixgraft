//! Convert merged configuration to command-line arguments.

use core::str::FromStr;

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use anyhow::Result;

/// Output format for command-line representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// JSON array of arguments.
    Json,
    /// Shell-escaped command ready to execute.
    Shell,
}

impl FromStr for OutputFormat {
    type Err = String;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shell" => Ok(Self::Shell),
            "json" => Ok(Self::Json),
            _ => Err(format!("Invalid format: {s}. Use 'shell' or 'json'")),
        }
    }
}

/// Convert configuration to command-line representation.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration cannot be converted to command-line arguments
/// - The command-line arguments cannot be serialized to the requested format
#[inline]
pub fn generate_command_line(config: &Config, format: OutputFormat) -> Result<String> {
    let args = build_command_args(config);

    match format {
        OutputFormat::Shell => Ok(format_as_shell(&args)),
        OutputFormat::Json => format_as_json(&args),
    }
}

/// Build argument list from configuration.
fn build_command_args(config: &Config) -> Vec<String> {
    let mut args = vec!["tixgraft".to_owned()];

    // Add global repository if specified
    if let Some(repo) = config.repository.as_ref() {
        args.push("--repository".to_owned());
        args.push(repo.clone());
    }

    // Add global tag if specified
    if let Some(tag) = config.tag.as_ref() {
        args.push("--tag".to_owned());
        args.push(tag.clone());
    }

    // Add each pull operation
    for pull in &config.pulls {
        add_pull_args(&mut args, pull, config);
    }

    args
}

/// Add arguments for a single pull operation.
fn add_pull_args(args: &mut Vec<String>, pull: &PullConfig, global_config: &Config) {
    // Source (required)
    args.push("--pull-source".to_owned());
    args.push(pull.source.clone());

    // Target (required)
    args.push("--pull-target".to_owned());
    args.push(pull.target.clone());

    // Type (only if not default)
    if pull.pull_type != "directory" {
        args.push("--pull-type".to_owned());
        args.push(pull.pull_type.clone());
    }

    // Repository (only if different from global)
    if let Some(repo) = pull.repository.as_ref()
        && global_config.repository.as_ref() != Some(repo)
    {
        args.push("--pull-repository".to_owned());
        args.push(repo.clone());
    }

    // Tag (only if different from global)
    if let Some(tag) = pull.tag.as_ref()
        && global_config.tag.as_ref() != Some(tag)
    {
        args.push("--pull-tag".to_owned());
        args.push(tag.clone());
    }

    // Reset (only if true)
    if pull.reset {
        args.push("--pull-reset".to_owned());
        args.push("true".to_owned());
    }

    // Require clean target (only emit when false, since true is the default)
    if !pull.require_clean_target {
        args.push("--pull-require-clean-target".to_owned());
        args.push("false".to_owned());
    }

    // Must succeed (only emit when false, since true is the default)
    if !pull.must_succeed {
        args.push("--pull-must-succeed".to_owned());
        args.push("false".to_owned());
    }

    // Replacements
    for replacement in &pull.replacements {
        args.push("--pull-replacement".to_owned());
        args.push(format_replacement(replacement));
    }

    // Commands
    for command in &pull.commands {
        args.push("--pull-commands".to_owned());
        args.push(command.clone());
    }
}

/// Format a replacement config as "SOURCE=TARGET" or "SOURCE=env:VAR".
fn format_replacement(repl: &ReplacementConfig) -> String {
    repl.value_from_env.as_ref().map_or_else(
        || {
            repl.target.as_ref().map_or_else(
                || {
                    // This shouldn't happen with valid config, but handle gracefully
                    repl.source.clone()
                },
                |target| format!("{}={}", repl.source, target),
            )
        },
        |env_var| format!("{}=env:{}", repl.source, env_var),
    )
}

/// Format arguments as a shell command with proper escaping.
fn format_as_shell(args: &[String]) -> String {
    let mut output = String::new();

    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            output.push_str(" \\\n  ");
        }

        // Shell escape the argument
        let escaped = shell_escape(arg);
        output.push_str(&escaped);
    }

    output
}

/// Format arguments as JSON array.
fn format_as_json(args: &[String]) -> Result<String> {
    serde_json::to_string_pretty(args)
        .map_err(|err| anyhow::anyhow!("Failed to serialize to JSON: {err}"))
}

/// Escape a string for shell execution.
/// Uses double quotes for safety, escaping special characters inside.
fn shell_escape(input: &str) -> String {
    // If string contains no special characters, return as-is
    if input.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '/' || ch == '.' || ch == ':'
    }) {
        return input.to_owned();
    }

    // Otherwise, wrap in double quotes and escape special chars
    let mut result = String::from('"');
    for ch in input.chars() {
        match ch {
            '"' => result.push_str(r#"\""#),
            '\\' => result.push_str(r"\\"),
            '$' => result.push_str(r"\$"),
            '`' => result.push_str(r"\`"),
            '!' => result.push_str(r"\!"),
            _ => result.push(ch),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests;

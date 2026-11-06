//! Renders a merged configuration's repository, tag and pulls as a `tixgraft` command line.
//!
//! The result is a shell-escaped command or a JSON array of arguments.

use core::str::FromStr;

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use anyhow::Result;

/// The rendering `--output-format` selects for `--to-command-line`.
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

/// Renders the repository, tag and pulls in the requested format; children and context values are
/// left out.
///
/// # Errors
///
/// Returns an error if the arguments cannot be serialized to JSON.
#[inline]
pub fn generate_command_line(config: &Config, format: OutputFormat) -> Result<String> {
    let args = build_command_args(config);

    match format {
        OutputFormat::Shell => Ok(format_as_shell(&args)),
        OutputFormat::Json => format_as_json(&args),
    }
}

/// Lists the program name, the global flags, then each pull's flags.
fn build_command_args(config: &Config) -> Vec<String> {
    let mut args = vec!["tixgraft".to_owned()];

    if let Some(repo) = config.repository.as_ref() {
        args.push("--repository".to_owned());
        args.push(repo.clone());
    }

    if let Some(tag) = config.tag.as_ref() {
        args.push("--tag".to_owned());
        args.push(tag.clone());
    }

    for pull in &config.pulls {
        add_pull_args(&mut args, pull, config);
    }

    args
}

/// Appends one pull's flags, leaving out any value equal to its default or the global setting.
fn add_pull_args(args: &mut Vec<String>, pull: &PullConfig, global_config: &Config) {
    args.push("--pull-source".to_owned());
    args.push(pull.source.clone());

    args.push("--pull-target".to_owned());
    args.push(pull.target.clone());

    if pull.pull_type != "directory" {
        args.push("--pull-type".to_owned());
        args.push(pull.pull_type.clone());
    }

    if let Some(repo) = pull.repository.as_ref()
        && global_config.repository.as_ref() != Some(repo)
    {
        args.push("--pull-repository".to_owned());
        args.push(repo.clone());
    }

    if let Some(tag) = pull.tag.as_ref()
        && global_config.tag.as_ref() != Some(tag)
    {
        args.push("--pull-tag".to_owned());
        args.push(tag.clone());
    }

    if pull.reset {
        args.push("--pull-reset".to_owned());
        args.push("true".to_owned());
    }

    if !pull.require_clean_target {
        args.push("--pull-require-clean-target".to_owned());
        args.push("false".to_owned());
    }

    if !pull.must_succeed {
        args.push("--pull-must-succeed".to_owned());
        args.push("false".to_owned());
    }

    for replacement in &pull.replacements {
        args.push("--pull-replacement".to_owned());
        args.push(format_replacement(replacement));
    }

    for command in &pull.commands {
        args.push("--pull-commands".to_owned());
        args.push(command.clone());
    }
}

/// Renders a replacement as `--pull-replacement` reads it: `SOURCE=TARGET` or `SOURCE=env:VAR`.
fn format_replacement(repl: &ReplacementConfig) -> String {
    repl.value_from_env.as_ref().map_or_else(
        || {
            repl.target.as_ref().map_or_else(
                || {
                    // Validation rejects a replacement with neither value; this keeps the source.
                    repl.source.clone()
                },
                |target| format!("{}={}", repl.source, target),
            )
        },
        |env_var| format!("{}=env:{}", repl.source, env_var),
    )
}

/// Joins the escaped arguments into one command, one argument per backslash-continued line.
fn format_as_shell(args: &[String]) -> String {
    let mut output = String::new();

    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            output.push_str(" \\\n  ");
        }

        let escaped = shell_escape(arg);
        output.push_str(&escaped);
    }

    output
}

/// Renders the arguments as a pretty-printed JSON array.
fn format_as_json(args: &[String]) -> Result<String> {
    serde_json::to_string_pretty(args)
        .map_err(|err| anyhow::anyhow!("Failed to serialize to JSON: {err}"))
}

/// Leaves a safe argument bare; any other goes in double quotes with active characters escaped.
fn shell_escape(input: &str) -> String {
    if input.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '/' || ch == '.' || ch == ':'
    }) {
        return input.to_owned();
    }

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

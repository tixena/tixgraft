//! Clap definition of the command line, and the YAML shape of a pull and its replacements.
//!
//! The doc comment on each clap field is that flag's `--help` text.

use std::collections::HashMap;

use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every flag tixgraft accepts: the run settings, the conversion modes, the context values and
/// the flattened pull and skill groups.
#[derive(Parser, Debug, Clone)]
#[command(name = "tixgraft")]
#[command(
    about = "A CLI tool for fetching reusable components from Git repositories or local paths"
)]
#[command(long_about = None)]
#[command(version)]
#[non_exhaustive]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI args naturally have many boolean flags"
)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order defines CLI help output order"
)]
pub struct Args {
    /// Source repository: Git URL, account/repo, or local path (file:..., ~/..., /abs, ./rel).
    #[arg(long, value_name = "REPO")]
    pub repository: Option<String>,

    /// Git reference (branch, tag, or commit hash).
    #[arg(long, value_name = "REF")]
    pub tag: Option<String>,

    /// Configuration file path.
    #[arg(long, value_name = "PATH", default_value = "./tixgraft.yaml")]
    pub config: String,

    /// Preview operations without executing.
    #[arg(long)]
    pub dry_run: bool,

    /// Enable verbose logging output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Output the equivalent command-line invocation instead of executing.
    #[arg(long = "to-command-line", conflicts_with = "to_config")]
    pub to_command_line: bool,

    /// Output the equivalent YAML configuration instead of executing.
    #[arg(long = "to-config", conflicts_with = "to_command_line")]
    pub to_config: bool,

    /// Output format for to-command-line: shell or json.
    #[arg(
        long = "output-format",
        value_name = "FORMAT",
        default_value = "shell",
        requires = "to_command_line"
    )]
    pub output_format: String,

    /// Context values in KEY=VALUE format (can be specified multiple times).
    /// Multiple values with the same key create an array.
    #[arg(long = "context", value_name = "KEY=VALUE")]
    pub context: Vec<String>,

    /// Context values as JSON in KEY=JSON format (can be specified multiple times).
    /// Use this for complex values like arrays of objects.
    #[arg(long = "context-json", value_name = "KEY=JSON")]
    pub context_json: Vec<String>,

    /// Pull operations (can be specified multiple times).
    #[command(flatten)]
    pub pulls: PullArgs,

    /// Skill management arguments.
    #[command(flatten)]
    pub skill: SkillArgs,
}

impl Args {
    /// Merges `--context` and `--context-json` into one map; a repeated key becomes an array.
    ///
    /// # Errors
    ///
    /// Returns an error if an argument has no `=` or a `--context-json` value is not valid JSON.
    #[inline]
    pub fn parse_context(&self) -> anyhow::Result<HashMap<String, Value>> {
        parse_context_args(&self.context, &self.context_json)
    }
}

/// The repeatable `--pull-*` flags: each value belongs to the `--pull-source` at its position,
/// except replacements, which reach every pull.
#[derive(Parser, Debug, Clone, Default)]
#[non_exhaustive]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order defines CLI help output order"
)]
pub struct PullArgs {
    /// Repository for this pull: Git URL, account/repo, or local path (file:..., ~/..., /abs, ./rel).
    #[arg(long = "pull-repository", value_name = "REPO")]
    pub repositories: Vec<String>,

    /// Git reference for specific pull.
    #[arg(long = "pull-tag", value_name = "REF")]
    pub tags: Vec<String>,

    /// Pull type: file or directory.
    #[arg(long = "pull-type", value_name = "TYPE", value_parser = ["file", "directory"])]
    pub types: Vec<String>,

    /// Source path within the repository or local folder.
    #[arg(long = "pull-source", value_name = "PATH")]
    pub sources: Vec<String>,

    /// Target path in local workspace.
    #[arg(long = "pull-target", value_name = "PATH")]
    pub targets: Vec<String>,

    /// Reset target directory before copying.
    #[arg(long = "pull-reset")]
    pub resets: Vec<bool>,

    /// Require clean git target directory before pulling (default: true).
    #[arg(long = "pull-require-clean-target")]
    pub require_clean_targets: Vec<bool>,

    /// Whether a pull failure is fatal (default: true). Set to false for optional pulls.
    #[arg(long = "pull-must-succeed")]
    pub must_succeeds: Vec<bool>,

    /// Commands to execute after copying.
    #[arg(long = "pull-commands", value_name = "COMMANDS")]
    pub commands: Vec<String>,

    /// Text replacements in format "SOURCE=TARGET" or "`SOURCE=env:ENV_VAR`".
    /// Can be specified multiple times per pull operation.
    #[arg(long = "pull-replacement", value_name = "REPLACEMENT")]
    pub replacements: Vec<String>,
}

/// The flags that install, remove or check the bundled skill, with `--global` and `--yes`
/// modifying them.
#[derive(Parser, Debug, Clone, Default)]
#[non_exhaustive]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order defines CLI help output order"
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "CLI args naturally have many boolean flags"
)]
pub struct SkillArgs {
    /// Install the tixgraft Claude Code skill.
    #[arg(long = "skill-install", conflicts_with_all = ["skill_uninstall", "skill_test", "to_command_line", "to_config"])]
    pub skill_install: bool,

    /// Uninstall the tixgraft Claude Code skill.
    #[arg(long = "skill-uninstall", conflicts_with_all = ["skill_install", "skill_test", "to_command_line", "to_config"])]
    pub skill_uninstall: bool,

    /// Test whether the tixgraft Claude Code skill is installed and up to date.
    #[arg(long = "skill-test", conflicts_with_all = ["skill_install", "skill_uninstall", "to_command_line", "to_config"])]
    pub skill_test: bool,

    /// Apply skill operation globally (~/.claude/skills/tixgraft/).
    #[arg(short = 'g', long = "global")]
    pub global: bool,

    /// Auto-confirm prompts (used with --skill-test).
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

/// One pull in the YAML configuration: what to fetch, where to put it and what to run on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order matches YAML config schema for readability"
)]
pub struct PullConfig {
    /// Path to copy, inside the repository or local folder.
    pub source: String,
    /// Local path to copy to.
    pub target: String,
    /// `file` or `directory`, which is the default.
    #[serde(default = "default_pull_type", rename = "type")]
    pub pull_type: String,
    /// Repository for this pull, overriding the global one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Git reference for this pull, overriding the global one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Removes a directory target before copying; a file pull ignores it.
    #[serde(default)]
    pub reset: bool,
    /// Refuses a target holding modified or untracked files in its Git repository; on by default.
    #[serde(default = "default_true", rename = "requireCleanTarget")]
    pub require_clean_target: bool,
    /// When false, a failure of this pull is logged and the run continues; on by default.
    #[serde(default = "default_true", rename = "mustSucceed")]
    pub must_succeed: bool,
    /// Shell commands run after copying, in the target or, for a file pull, in its parent.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Text replacements applied to the copied files.
    #[serde(default)]
    pub replacements: Vec<ReplacementConfig>,
    /// Context values for this pull, overriding the global ones.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,
}

/// One text replacement: the text to search for and where its new value comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReplacementConfig {
    /// Literal text to search for.
    pub source: String,
    /// Replacement text; exclusive with `value_from_env`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Environment variable holding the replacement text; exclusive with `target`.
    #[serde(rename = "valueFromEnv", skip_serializing_if = "Option::is_none")]
    pub value_from_env: Option<String>,
}

impl ReplacementConfig {
    /// Builds a replacement; validation later requires exactly one of the two value sources.
    #[must_use]
    #[inline]
    pub const fn new(
        source: String,
        target: Option<String>,
        value_from_env: Option<String>,
    ) -> Self {
        Self {
            source,
            target,
            value_from_env,
        }
    }
}

/// Serde default of a pull's `type` field: a directory.
fn default_pull_type() -> String {
    "directory".to_owned()
}

/// Serde default of the pull flags that stay on unless the YAML turns them off.
const fn default_true() -> bool {
    true
}

/// Groups both flags' `KEY=VALUE` pairs by key: one value stays as it is, several become an array.
#[expect(
    clippy::iter_over_hash_type,
    reason = "iteration order does not matter for context key-value collection"
)]
fn parse_context_args(
    context_args: &[String],
    context_json_args: &[String],
) -> anyhow::Result<HashMap<String, Value>> {
    let mut result: HashMap<String, Vec<Value>> = HashMap::new();

    for arg in context_args {
        let (key, value) = parse_key_value(arg)?;
        result.entry(key).or_default().push(Value::String(value));
    }

    for arg in context_json_args {
        let (key, json_str) = parse_key_value(arg)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|err| {
            anyhow::anyhow!(
                "Invalid JSON in --context-json for key '{key}': {err}\nValue: {json_str}"
            )
        })?;
        result.entry(key).or_default().push(value);
    }

    let mut final_result = HashMap::new();
    for (key, values) in result {
        if values.len() == 1 {
            final_result.insert(
                key,
                values
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("No value found for key"))?,
            );
        } else {
            final_result.insert(key, Value::Array(values));
        }
    }

    Ok(final_result)
}

/// Splits at the first `=`, so the value may itself contain `=`.
#[expect(
    clippy::indexing_slicing,
    reason = "parts length is checked to be exactly 2 before indexing"
)]
fn parse_key_value(arg: &str) -> anyhow::Result<(String, String)> {
    let parts: Vec<&str> = arg.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid context format '{arg}'. Expected KEY=VALUE"
        ));
    }
    Ok((parts[0].to_owned(), parts[1].to_owned()))
}

#[cfg(test)]
mod tests;

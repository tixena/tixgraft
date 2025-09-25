use clap::Parser;
use serde::{Deserialize, Serialize};

/// Command-line arguments for tixgraft
#[derive(Parser, Debug, Clone)]
#[command(name = "tixgraft")]
#[command(about = "A CLI tool for fetching reusable components from Git repositories")]
#[command(long_about = None)]
#[command(version)]
pub struct Args {
    /// Git repository URL or account/repo format
    #[arg(long, value_name = "REPO")]
    pub repository: Option<String>,

    /// Git reference (branch, tag, or commit hash)
    #[arg(long, value_name = "REF")]
    pub tag: Option<String>,

    /// Configuration file path
    #[arg(long, value_name = "PATH", default_value = "./tixgraft.yaml")]
    pub config: String,

    /// Preview operations without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Pull operations (can be specified multiple times)
    #[command(flatten)]
    pub pulls: PullArgs,
}

/// Arguments for individual pull operations
#[derive(Parser, Debug, Clone, Default)]
pub struct PullArgs {
    /// Repository for specific pull
    #[arg(long = "pull-repository", value_name = "REPO")]
    pub repositories: Vec<String>,

    /// Git reference for specific pull
    #[arg(long = "pull-tag", value_name = "REF")]
    pub tags: Vec<String>,

    /// Pull type: file or directory
    #[arg(long = "pull-type", value_name = "TYPE", value_parser = ["file", "directory"])]
    pub types: Vec<String>,

    /// Source path in Git repository
    #[arg(long = "pull-source", value_name = "PATH")]
    pub sources: Vec<String>,

    /// Target path in local workspace
    #[arg(long = "pull-target", value_name = "PATH")]
    pub targets: Vec<String>,

    /// Reset target directory before copying
    #[arg(long = "pull-reset")]
    pub resets: Vec<bool>,

    /// Commands to execute after copying
    #[arg(long = "pull-commands", value_name = "COMMANDS")]
    pub commands: Vec<String>,
}

/// Individual pull configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullConfig {
    pub source: String,
    pub target: String,
    #[serde(default = "default_pull_type", rename = "type")]
    pub pull_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default)]
    pub reset: bool,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub replacements: Vec<ReplacementConfig>,
}

/// Text replacement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementConfig {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(rename = "valueFromEnv", skip_serializing_if = "Option::is_none")]
    pub value_from_env: Option<String>,
}

fn default_pull_type() -> String {
    "directory".to_string()
}

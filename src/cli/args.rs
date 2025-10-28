use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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

    /// Enable verbose logging output
    #[arg(short, long)]
    pub verbose: bool,

    /// Output the equivalent command-line invocation instead of executing
    #[arg(long = "to-command-line", conflicts_with = "to_config")]
    pub to_command_line: bool,

    /// Output the equivalent YAML configuration instead of executing
    #[arg(long = "to-config", conflicts_with = "to_command_line")]
    pub to_config: bool,

    /// Output format for to-command-line: shell or json
    #[arg(
        long = "output-format",
        value_name = "FORMAT",
        default_value = "shell",
        requires = "to_command_line"
    )]
    pub output_format: String,

    /// Context values in KEY=VALUE format (can be specified multiple times)
    /// Multiple values with the same key create an array
    #[arg(long = "context", value_name = "KEY=VALUE")]
    pub context: Vec<String>,

    /// Context values as JSON in KEY=JSON format (can be specified multiple times)
    /// Use this for complex values like arrays of objects
    #[arg(long = "context-json", value_name = "KEY=JSON")]
    pub context_json: Vec<String>,

    /// Pull operations (can be specified multiple times)
    #[command(flatten)]
    pub pulls: PullArgs,
}

impl Args {
    /// Parse context arguments into a `HashMap`
    pub fn parse_context(&self) -> anyhow::Result<HashMap<String, Value>> {
        parse_context_args(&self.context, &self.context_json)
    }
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

    /// Text replacements in format "SOURCE=TARGET" or "`SOURCE=env:ENV_VAR`"
    /// Can be specified multiple times per pull operation
    #[arg(long = "pull-replacement", value_name = "REPLACEMENT")]
    pub replacements: Vec<String>,
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
    /// Context values for this pull
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,
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
    return "directory".to_owned();
}

/// Parse context arguments from CLI into a `HashMap`
/// Handles both --context and --context-json flags
/// Multiple values with the same key create an array
fn parse_context_args(
    context_args: &[String],
    context_json_args: &[String],
) -> anyhow::Result<HashMap<String, Value>> {
    let mut result: HashMap<String, Vec<Value>> = HashMap::new();

    // Parse --context arguments
    for arg in context_args {
        let (key, value) = parse_key_value(arg)?;
        result.entry(key).or_default().push(Value::String(value));
    }

    // Parse --context-json arguments
    for arg in context_json_args {
        let (key, json_str) = parse_key_value(arg)?;
        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            return anyhow::anyhow!(
                "Invalid JSON in --context-json for key '{key}': {e}\nValue: {json_str}"
            );
        })?;
        result.entry(key).or_default().push(value);
    }

    // Convert Vec<Value> to Value (single value or array)
    let mut final_result = HashMap::new();
    for (key, values) in result {
        if values.len() == 1 {
            final_result.insert(key, values.into_iter().next().unwrap());
        } else {
            final_result.insert(key, Value::Array(values));
        }
    }

    Ok(final_result)
}

/// Parse KEY=VALUE string
fn parse_key_value(arg: &str) -> anyhow::Result<(String, String)> {
    let parts: Vec<&str> = arg.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid context format '{arg}'. Expected KEY=VALUE"
        ));
    }
    return Ok((parts[0].to_owned(), parts[1].to_owned()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_context() {
        let context = vec!["name=test".to_string(), "port=8080".to_string()];
        let json = vec![];
        let result = parse_context_args(&context, &json).unwrap();

        assert_eq!(result.get("name"), Some(&Value::String("test".to_string())));
        assert_eq!(result.get("port"), Some(&Value::String("8080".to_string())));
    }

    #[test]
    fn test_parse_array_context() {
        let context = vec![
            "items=a".to_string(),
            "items=b".to_string(),
            "items=c".to_string(),
        ];
        let json = vec![];
        let result = parse_context_args(&context, &json).unwrap();

        assert_eq!(
            result.get("items"),
            Some(&Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string())
            ]))
        );
    }

    #[test]
    fn test_parse_json_context() {
        let context = vec![];
        let json = vec![r#"config={"key":"value"}"#.to_string()];
        let result = parse_context_args(&context, &json).unwrap();

        let expected = serde_json::json!({"key": "value"});
        assert_eq!(result.get("config"), Some(&expected));
    }

    #[test]
    fn test_parse_mixed_context() {
        let context = vec!["name=test".to_string()];
        let json = vec![r#"people=[{"name":"Alice"},{"name":"Bob"}]"#.to_string()];
        let result = parse_context_args(&context, &json).unwrap();

        assert_eq!(result.get("name"), Some(&Value::String("test".to_string())));
        assert_eq!(
            result.get("people"),
            Some(&serde_json::json!([{"name":"Alice"},{"name":"Bob"}]))
        );
    }

    #[test]
    fn test_invalid_context_format() {
        let context = vec!["invalid".to_string()];
        let json = vec![];
        let result = parse_context_args(&context, &json);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Expected KEY=VALUE")
        );
    }

    #[test]
    fn test_invalid_json() {
        let context = vec![];
        let json = vec![r#"config={invalid json}"#.to_string()];
        let result = parse_context_args(&context, &json);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }
}

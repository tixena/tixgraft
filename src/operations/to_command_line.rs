//! Convert merged configuration to command-line arguments

use core::str::FromStr;

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use anyhow::Result;

/// Output format for command-line representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// Shell-escaped command ready to execute
    Shell,
    /// JSON array of arguments
    Json,
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

/// Convert configuration to command-line representation
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

/// Build argument list from configuration
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

/// Add arguments for a single pull operation
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

/// Format a replacement config as "SOURCE=TARGET" or "SOURCE=env:VAR"
fn format_replacement(repl: &ReplacementConfig) -> String {
    if let Some(env_var) = repl.value_from_env.as_ref() {
        format!("{}=env:{}", repl.source, env_var)
    } else if let Some(target) = repl.target.as_ref() {
        format!("{}={}", repl.source, target)
    } else {
        // This shouldn't happen with valid config, but handle gracefully
        repl.source.clone()
    }
}

/// Format arguments as a shell command with proper escaping
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

/// Format arguments as JSON array
fn format_as_json(args: &[String]) -> Result<String> {
    serde_json::to_string_pretty(args)
        .map_err(|e| anyhow::anyhow!("Failed to serialize to JSON: {e}"))
}

/// Escape a string for shell execution
/// Uses double quotes for safety, escaping special characters inside
fn shell_escape(s: &str) -> String {
    // If string contains no special characters, return as-is
    if s.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.' || c == ':'
    }) {
        return s.to_owned();
    }

    // Otherwise, wrap in double quotes and escape special chars
    let mut result = String::from('"');
    for ch in s.chars() {
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
#[expect(clippy::unwrap_used, reason = "These are unit tests")]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("path/to/file"), "path/to/file");
        assert_eq!(shell_escape("file.txt"), "file.txt");
        assert_eq!(shell_escape("repo-name"), "repo-name");
    }

    #[test]
    fn shell_escape_special_chars() {
        assert_eq!(shell_escape("has space"), r#""has space""#);
        assert_eq!(shell_escape("has$dollar"), r#""has\$dollar""#);
        assert_eq!(shell_escape(r#"has"quote"#), r#""has\"quote""#);
        assert_eq!(shell_escape("back\\slash"), r#""back\\slash""#);
    }

    #[test]
    fn format_replacement_tst() {
        let repl_static = ReplacementConfig {
            source: "{{VAR}}".to_owned(),
            target: Some("value".to_owned()),
            value_from_env: None,
        };
        assert_eq!(format_replacement(&repl_static), "{{VAR}}=value");

        let repl_env = ReplacementConfig {
            source: "{{VAR}}".to_owned(),
            target: None,
            value_from_env: Some("MY_ENV".to_owned()),
        };
        assert_eq!(format_replacement(&repl_env), "{{VAR}}=env:MY_ENV");
    }

    #[test]
    fn output_format_from_str() {
        assert_eq!(
            "shell".parse::<OutputFormat>().unwrap(),
            OutputFormat::Shell
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "SHELL".parse::<OutputFormat>().unwrap(),
            OutputFormat::Shell
        );
        "invalid".parse::<OutputFormat>().unwrap_err();
    }

    #[test]
    fn build_command_args_basic() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: Some("main".to_owned()),
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "src".to_owned(),
                target: "dst".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None,
                tag: None,
                reset: false,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            }],
        };

        let args = build_command_args(&config);
        assert_eq!(args[0], "tixgraft");
        assert!(args.contains(&"--repository".to_owned()));
        assert!(args.contains(&"my_organization/repo".to_owned()));
        assert!(args.contains(&"--tag".to_owned()));
        assert!(args.contains(&"main".to_owned()));
        assert!(args.contains(&"--pull-source".to_owned()));
        assert!(args.contains(&"src".to_owned()));
        assert!(args.contains(&"--pull-target".to_owned()));
        assert!(args.contains(&"dst".to_owned()));
    }

    #[test]
    fn build_command_args_with_reset() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: None,
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "src".to_owned(),
                target: "dst".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None,
                tag: None,
                reset: true,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            }],
        };

        let args = build_command_args(&config);
        assert!(args.contains(&"--pull-reset".to_owned()));
    }

    #[test]
    fn build_command_args_with_replacements() {
        use crate::cli::{PullConfig, ReplacementConfig};

        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: None,
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "src".to_owned(),
                target: "dst".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None,
                tag: None,
                reset: false,
                commands: vec![],
                replacements: vec![
                    ReplacementConfig {
                        source: "{{VAR1}}".to_owned(),
                        target: Some("value1".to_owned()),
                        value_from_env: None,
                    },
                    ReplacementConfig {
                        source: "{{VAR2}}".to_owned(),
                        target: None,
                        value_from_env: Some("MY_ENV".to_owned()),
                    },
                ],
                context: HashMap::new(),
            }],
        };

        let args = build_command_args(&config);
        assert!(args.contains(&"--pull-replacement".to_owned()));
        assert!(args.contains(&"{{VAR1}}=value1".to_owned()));
        assert!(args.contains(&"{{VAR2}}=env:MY_ENV".to_owned()));
    }

    #[test]
    fn multiline_command() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("repo".to_owned()),
            tag: None,
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "src".to_owned(),
                target: "dst".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None,
                tag: None,
                reset: false,
                commands: vec!["echo 'line1'\necho 'line2'".to_owned()],
                replacements: vec![],
                context: HashMap::new(),
            }],
        };

        let result = generate_command_line(&config, OutputFormat::Shell);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should escape the newline properly
        assert!(output.contains("--pull-commands"));
    }

    #[test]
    fn replacement_with_special_chars() {
        use crate::cli::ReplacementConfig;

        let replacement = ReplacementConfig {
            source: "{{VAR}}".to_owned(),
            target: Some(r#"value with "quotes" and $vars"#.to_owned()),
            value_from_env: None,
        };

        let formatted = format_replacement(&replacement);
        assert_eq!(formatted, r#"{{VAR}}=value with "quotes" and $vars"#);

        // Now test that shell_escape properly escapes it
        let escaped = shell_escape(&formatted);
        assert!(escaped.contains(r#"\""#)); // Quotes should be escaped
        assert!(escaped.contains(r"\$")); // Dollar signs should be escaped
    }

    #[test]
    fn replacement_with_newlines() {
        use crate::cli::ReplacementConfig;

        let replacement = ReplacementConfig {
            source: "{{VAR}}".to_owned(),
            target: Some("line1\nline2".to_owned()),
            value_from_env: None,
        };

        let formatted = format_replacement(&replacement);
        assert_eq!(formatted, "{{VAR}}=line1\nline2");

        // Shell escape should handle newlines
        let escaped = shell_escape(&formatted);
        assert!(escaped.starts_with('"'));
        assert!(escaped.ends_with('"'));
    }

    #[test]
    fn empty_pulls_array() {
        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: Some("main".to_owned()),
            context: HashMap::new(),
            pulls: vec![],
        };

        let args = build_command_args(&config);
        // Should still work, just no pull args
        assert_eq!(args[0], "tixgraft");
        assert!(args.contains(&"--repository".to_owned()));
        assert!(args.contains(&"my_organization/repo".to_owned()));
    }

    #[test]
    fn path_with_spaces() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: None,
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "src with spaces".to_owned(),
                target: "dst with spaces".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None,
                tag: None,
                reset: false,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            }],
        };

        let result = generate_command_line(&config, OutputFormat::Shell);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Paths with spaces should be quoted
        assert!(output.contains(r#""src with spaces""#));
        assert!(output.contains(r#""dst with spaces""#));
    }

    #[test]
    fn file_type_pull() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("my_organization/repo".to_owned()),
            tag: None,
            context: HashMap::new(),
            pulls: vec![PullConfig {
                source: "file.txt".to_owned(),
                target: "output.txt".to_owned(),
                pull_type: "file".to_owned(),
                repository: None,
                tag: None,
                reset: false,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            }],
        };

        let args = build_command_args(&config);
        // File type should be included since it's not the default
        assert!(args.contains(&"--pull-type".to_owned()));
        assert!(args.contains(&"file".to_owned()));
    }

    #[test]
    fn per_pull_overrides() {
        use crate::cli::PullConfig;

        let config = Config {
            repository: Some("global/repo".to_owned()),
            tag: Some("v1".to_owned()),
            context: HashMap::new(),
            pulls: vec![
                PullConfig {
                    source: "src1".to_owned(),
                    target: "dst1".to_owned(),
                    pull_type: "directory".to_owned(),
                    repository: None, // Uses global
                    tag: None,        // Uses global
                    reset: false,
                    commands: vec![],
                    replacements: vec![],
                    context: HashMap::new(),
                },
                PullConfig {
                    source: "src2".to_owned(),
                    target: "dst2".to_owned(),
                    pull_type: "directory".to_owned(),
                    repository: Some("per-pull/repo".to_owned()), // Override
                    tag: Some("v2".to_owned()),                   // Override
                    reset: false,
                    commands: vec![],
                    replacements: vec![],
                    context: HashMap::new(),
                },
            ],
        };

        let args = build_command_args(&config);
        // Should have per-pull overrides for the second pull
        assert!(args.contains(&"--pull-repository".to_owned()));
        assert!(args.contains(&"per-pull/repo".to_owned()));
        assert!(args.contains(&"--pull-tag".to_owned()));
        assert!(args.contains(&"v2".to_owned()));
    }
}

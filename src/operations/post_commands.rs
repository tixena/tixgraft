//! Post-command execution for .graft.yaml files
//!
//! Handles execution of commands after graft processing, including
//! simple commands and conditional choice-based execution.

use crate::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
use crate::error::GraftError;
use anyhow::{Context as _, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// Execute all post-commands in order
///
/// Commands execute in the directory containing the .graft.yaml file
pub fn execute_post_commands(
    commands: &[PostCommand],
    graft_directory: &Path,
) -> Result<Vec<ExecutionResult>> {
    let mut results = Vec::new();

    for command in commands {
        let result = execute_post_command(command, graft_directory)?;
        results.push(result);
    }

    Ok(results)
}

/// Result of executing a post-command
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub command_type: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Execute a single post-command
fn execute_post_command(command: &PostCommand, graft_directory: &Path) -> Result<ExecutionResult> {
    match command {
        PostCommand::Command { command, args, cwd } => {
            execute_simple_command(command, args, cwd.as_deref(), graft_directory)
        }
        PostCommand::Choice { options } => execute_choice(options, graft_directory),
    }
}

/// Execute a simple command
fn execute_simple_command(
    command: &str,
    args: &[String],
    cwd: Option<&str>,
    graft_directory: &Path,
) -> Result<ExecutionResult> {
    let working_dir = resolve_working_directory(cwd, graft_directory)?;

    let output = Command::new(command)
        .args(args)
        .current_dir(&working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "Failed to execute command '{}' in directory '{}'",
                command,
                working_dir.display()
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    return Ok(ExecutionResult {
        command_type: "command".to_owned(),
        success: output.status.success(),
        output: stdout,
        error: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
    });
}

/// Execute a conditional choice
fn execute_choice(options: &[ChoiceOption], graft_directory: &Path) -> Result<ExecutionResult> {
    // Try each option in order
    for option in options {
        let test_result = execute_test_command(&option.test, graft_directory)?;

        // Check if output matches expected
        if test_result.output.contains(&option.expected_output) {
            // Match found, execute the onMatch command
            return execute_post_command(&option.on_match, graft_directory);
        }
    }

    // No matches found, return a no-op result
    return Ok(ExecutionResult {
        command_type: "choice".to_owned(),
        success: true,
        output: "No matching option found".to_owned(),
        error: None,
    });
}

/// Execute a test command (for choice conditions)
fn execute_test_command(test: &TestCommand, graft_directory: &Path) -> Result<ExecutionResult> {
    let working_dir = resolve_working_directory(test.cwd.as_deref(), graft_directory)?;

    let output = Command::new(&test.command)
        .args(&test.args)
        .current_dir(&working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "Failed to execute test command '{}' in directory '{}'",
                test.command,
                working_dir.display()
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    return Ok(ExecutionResult {
        command_type: "test".to_owned(),
        success: output.status.success(),
        output: stdout,
        error: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
    });
}

/// Resolve the working directory for command execution
///
/// If cwd is None, uses `graft_directory`
/// If cwd is Some, resolves it relative to `graft_directory`
fn resolve_working_directory(
    cwd: Option<&str>,
    graft_directory: &Path,
) -> Result<std::path::PathBuf> {
    if let Some(cwd_str) = cwd {
        let cwd_path = Path::new(cwd_str);

        // If absolute, use as-is
        if cwd_path.is_absolute() {
            return Ok(cwd_path.to_path_buf());
        }

        // Otherwise, resolve relative to graft_directory
        let resolved = graft_directory.join(cwd_path);

        if !resolved.exists() {
            return Err(GraftError::configuration(format!(
                "Working directory does not exist: {}",
                resolved.display()
            ))
            .into());
        }

        Ok(resolved)
    } else {
        Ok(graft_directory.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_execute_simple_command() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Command {
            command: "echo".to_string(),
            args: vec!["Hello, World!".to_string()],
            cwd: None,
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello, World!"));
    }

    #[test]
    fn test_execute_command_with_cwd() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Create a file in subdir to verify cwd
        fs::write(sub_dir.join("test.txt"), "content").unwrap();

        let command = PostCommand::Command {
            command: "ls".to_string(),
            args: vec![],
            cwd: Some("subdir".to_string()),
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
    }

    #[test]
    fn test_execute_choice_with_match() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["version 1.0".to_string()],
                    cwd: None,
                },
                expected_output: "version".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["matched!".to_string()],
                    cwd: None,
                }),
            }],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("matched!"));
    }

    #[test]
    fn test_execute_choice_no_match() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["hello".to_string()],
                    cwd: None,
                },
                expected_output: "version".to_string(), // Won't match "hello"
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["should not run".to_string()],
                    cwd: None,
                }),
            }],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matching option"));
    }

    #[test]
    fn test_execute_nested_choice() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["outer".to_string()],
                    cwd: None,
                },
                expected_output: "outer".to_string(),
                on_match: Box::new(PostCommand::Choice {
                    options: vec![ChoiceOption {
                        test: TestCommand {
                            command: "echo".to_string(),
                            args: vec!["inner".to_string()],
                            cwd: None,
                        },
                        expected_output: "inner".to_string(),
                        on_match: Box::new(PostCommand::Command {
                            command: "echo".to_string(),
                            args: vec!["nested match!".to_string()],
                            cwd: None,
                        }),
                    }],
                }),
            }],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("nested match!"));
    }

    #[test]
    fn test_resolve_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Test None (uses graft_directory)
        let result = resolve_working_directory(None, temp_dir.path()).unwrap();
        assert_eq!(result, temp_dir.path());

        // Test relative path
        let result = resolve_working_directory(Some("subdir"), temp_dir.path()).unwrap();
        assert_eq!(result, sub_dir);

        // Test non-existent directory
        let result = resolve_working_directory(Some("nonexistent"), temp_dir.path());
        assert!(result.is_err());
    }
}

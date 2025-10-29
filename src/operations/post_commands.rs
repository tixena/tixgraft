//! Post-command execution for .graft.yaml files
//!
//! Handles execution of commands after graft processing, including
//! simple commands and conditional choice-based execution.

use crate::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
use crate::error::GraftError;
use anyhow::{Context as _, Result};
use regex::Regex;
use std::path::Path;
use std::process::{Command, Stdio};

/// Execute all post-commands in order
///
/// Commands execute in the directory containing the .graft.yaml file
/// Continues executing all commands even if some fail, collecting all results
pub fn execute_post_commands(
    commands: &[PostCommand],
    graft_directory: &Path,
) -> Result<Vec<ExecutionResult>> {
    let mut results = Vec::new();

    for command in commands {
        let result = match execute_post_command(command, graft_directory) {
            Ok(result) => result,
            Err(err) => {
                // Convert execution errors into failed ExecutionResult
                ExecutionResult {
                    command_type: "command".to_owned(),
                    success: false,
                    output: String::new(),
                    error: Some(format!("{:#}", err)),
                }
            }
        };
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
pub fn execute_post_command(
    command: &PostCommand,
    graft_directory: &Path,
) -> Result<ExecutionResult> {
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
///
/// Tests each option's command and matches the output against a regex pattern.
/// The first matching option's `onMatch` command is executed.
///
/// # Pattern Matching
///
/// The `expectedOutput` field is treated as a regular expression pattern.
/// Simple strings like "version" will match anywhere in the output.
/// More complex patterns like "^v\\d+\\.\\d+\\.\\d+$" can be used for precise matching.
fn execute_choice(options: &[ChoiceOption], graft_directory: &Path) -> Result<ExecutionResult> {
    // Try each option in order
    for option in options {
        let test_result = execute_test_command(&option.test, graft_directory)?;

        // Check if output matches expected pattern (regex)
        let pattern = Regex::new(&option.expected_output).with_context(|| {
            format!(
                "Invalid regex pattern in expectedOutput: '{}'",
                option.expected_output
            )
        })?;

        if pattern.is_match(&test_result.output) {
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
pub fn resolve_working_directory(
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

//! Runs the post-commands a `.graft.yaml` declares, once its replacements are applied.
//!
//! A post-command runs directly with no shell, or is chosen by matching a test command's output.

use crate::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
use crate::error::GraftError;
use anyhow::{Context as _, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The outcome of one post-command, failure included, for the caller to report.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExecutionResult {
    /// `command`, `test`, or `choice` when no option matched.
    pub command_type: String,
    /// The command's stderr when it wrote any, or the reason it could not run.
    pub error: Option<String>,
    /// The command's stdout, lossily decoded, or a note when no choice option matched.
    pub output: String,
    /// Whether the exit status was success; a choice that matched no option counts as success.
    pub success: bool,
}

/// Runs every post-command in order from the `.graft.yaml` directory, one result each; a failing
/// command does not stop the rest.
///
/// # Errors
///
/// Always returns `Ok`: a command that fails is recorded in its own result.
#[inline]
#[expect(clippy::module_name_repetitions, reason = "PostCommands")]
pub fn execute_post_commands(
    commands: &[PostCommand],
    graft_directory: &Path,
) -> Result<Vec<ExecutionResult>> {
    let mut results = Vec::new();

    for command in commands {
        let result = match execute_post_command(command, graft_directory) {
            Ok(result) => result,
            Err(err) => {
                // A launch failure becomes a failed result, so the remaining commands still run.
                ExecutionResult {
                    command_type: "command".to_owned(),
                    error: Some(format!("{err:#}")),
                    output: String::new(),
                    success: false,
                }
            }
        };
        results.push(result);
    }

    Ok(results)
}

/// Runs one post-command, directly or through the option its choice selects.
///
/// # Errors
///
/// Returns an error if a command cannot start, a directory is missing or a regex is invalid.
#[inline]
#[expect(
    clippy::ref_patterns,
    reason = "ref is required to borrow fields when matching on a dereferenced enum to satisfy pattern_type_mismatch"
)]
pub fn execute_post_command(
    post_command: &PostCommand,
    graft_directory: &Path,
) -> Result<ExecutionResult> {
    match *post_command {
        PostCommand::Command {
            ref command,
            ref args,
            ref cwd,
        } => execute_simple_command(command, args, cwd.as_deref(), graft_directory),
        PostCommand::Choice { ref options } => execute_choice(options, graft_directory),
    }
}

/// Joins a relative `cwd` onto `graft_directory`; an absolute `cwd` is returned unchecked and no
/// `cwd` means `graft_directory` itself.
///
/// # Errors
///
/// Returns an error if a relative `cwd` resolves to a missing directory.
#[inline]
pub fn resolve_working_directory(cwd: Option<&str>, graft_directory: &Path) -> Result<PathBuf> {
    if let Some(cwd_str) = cwd {
        let cwd_path = Path::new(cwd_str);

        if cwd_path.is_absolute() {
            return Ok(cwd_path.to_path_buf());
        }

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

/// Runs each option's test command in order, then the `onMatch` of the first whose stdout matches
/// its `expectedOutput` regex anywhere. No match is a success that runs nothing.
fn execute_choice(options: &[ChoiceOption], graft_directory: &Path) -> Result<ExecutionResult> {
    for option in options {
        let test_result = execute_test_command(&option.test, graft_directory)?;

        let pattern = Regex::new(&option.expected_output).with_context(|| {
            format!(
                "Invalid regex pattern in expectedOutput: '{}'",
                option.expected_output
            )
        })?;

        if pattern.is_match(&test_result.output) {
            return execute_post_command(&option.on_match, graft_directory);
        }
    }

    Ok(ExecutionResult {
        command_type: "choice".to_owned(),
        error: None,
        output: "No matching option found".to_owned(),
        success: true,
    })
}

/// Runs `command` directly, with no shell, in its resolved directory and captures both streams.
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

    Ok(ExecutionResult {
        command_type: "command".to_owned(),
        error: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
        output: stdout,
        success: output.status.success(),
    })
}

/// Runs a choice option's test command with no shell; `execute_choice` reads only its stdout.
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

    Ok(ExecutionResult {
        command_type: "test".to_owned(),
        error: if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        },
        output: stdout,
        success: output.status.success(),
    })
}

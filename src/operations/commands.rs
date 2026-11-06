//! Runs a pull's shell commands in a working directory, captured or attached to the terminal.
//!
//! Also flags, without running them, the commands that look destructive or reach the network.

use crate::error::GraftError;
use anyhow::{Context as _, Result};
use core::fmt::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::info;

/// One command's verdict from `validate_commands`, which never runs it.
#[derive(Debug)]
#[non_exhaustive]
pub struct CommandValidation {
    /// The command text as given.
    pub command: String,
    /// Its 1-based position in the list.
    pub command_number: usize,
    /// False when the command is empty or only whitespace.
    pub is_valid: bool,
    /// A message for each risk found, none of which affects `is_valid`.
    pub potential_issues: Vec<String>,
}

/// Runs the commands in order with captured output, stopping at the first failure, and returns how
/// many ran.
///
/// # Errors
///
/// Returns an error if the working directory is missing or a command is empty or fails.
#[inline]
pub fn execute_commands(commands: &[String], working_dir: &str) -> Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }

    let work_path = Path::new(working_dir);
    if !work_path.exists() {
        return Err(GraftError::filesystem(format!(
            "Working directory does not exist: {working_dir}"
        ))
        .into());
    }

    let mut executed_count: usize = 0;

    for (index, command) in commands.iter().enumerate() {
        execute_single_command(command, work_path, index.saturating_add(1))?;
        executed_count = executed_count.saturating_add(1);
    }

    Ok(executed_count)
}

/// Runs the commands in order attached to the terminal, stopping at the first failure, and returns
/// how many ran.
///
/// # Errors
///
/// Returns an error if the working directory is missing or a command is empty or fails.
#[inline]
pub fn execute_commands_interactive(commands: &[String], working_dir: &str) -> Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }

    let work_path = Path::new(working_dir);
    if !work_path.exists() {
        return Err(GraftError::filesystem(format!(
            "Working directory does not exist: {working_dir}"
        ))
        .into());
    }

    let mut executed_count: usize = 0;

    for (index, command) in commands.iter().enumerate() {
        execute_single_command_interactive(command, work_path, index.saturating_add(1))?;
        executed_count = executed_count.saturating_add(1);
    }

    Ok(executed_count)
}

/// Checks each command for emptiness and risky patterns, without running any.
///
/// # Errors
///
/// Always returns `Ok`: every command receives a verdict.
#[inline]
pub fn validate_commands(commands: &[String]) -> Result<Vec<CommandValidation>> {
    let mut validations = Vec::new();

    for (index, command) in commands.iter().enumerate() {
        let validation = CommandValidation {
            command: command.clone(),
            command_number: index.saturating_add(1),
            is_valid: !command.trim().is_empty(),
            potential_issues: analyze_command_safety(command),
        };

        validations.push(validation);
    }

    Ok(validations)
}

/// Flags by substring: destructive commands, a fork bomb among them, network tools, `eval`, `exec`.
fn analyze_command_safety(command: &str) -> Vec<String> {
    let mut issues = Vec::new();

    let dangerous_patterns = [
        "rm -rf",
        "rm -f",
        "format",
        "mkfs",
        "dd if=",
        ":(){ :|:& };:",
        "sudo rm",
    ];

    for pattern in &dangerous_patterns {
        if command.to_lowercase().contains(&pattern.to_lowercase()) {
            issues.push(format!(
                "Contains potentially destructive command: {pattern}"
            ));
        }
    }

    let network_patterns = ["curl", "wget", "nc ", "netcat"];

    for pattern in &network_patterns {
        if command.to_lowercase().contains(&pattern.to_lowercase()) {
            issues.push("Command may access network resources".to_owned());
            break;
        }
    }

    if command.contains("eval") || command.contains("exec") {
        issues.push("Command contains script execution which could be risky".to_owned());
    }

    issues
}

/// Runs one command through the shell with captured output, reporting both streams on failure.
fn execute_single_command(command: &str, working_dir: &Path, command_number: usize) -> Result<()> {
    if command.trim().is_empty() {
        return Err(GraftError::command(format!("Command #{command_number} is empty")).into());
    }

    let (shell, shell_args) = get_shell_command();
    let mut cmd_args = shell_args;
    cmd_args.push(command.to_owned());

    let output = Command::new(&shell)
        .args(&cmd_args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute command #{command_number}: {command}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut error_msg = format!(
            "Command #{} failed with exit code {}: {}\n",
            command_number,
            output.status.code().unwrap_or(-1_i32),
            command
        );

        if !stderr.trim().is_empty() {
            write!(error_msg, "Error output:\n{}\n", stderr.trim())?;
        }

        if !stdout.trim().is_empty() {
            write!(error_msg, "Standard output:\n{}\n", stdout.trim())?;
        }

        write!(error_msg, "Working directory: {}", working_dir.display())?;

        return Err(GraftError::command(error_msg).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        info!("Command #{} output:", command_number);
        info!("{}", stdout.trim());
    }

    Ok(())
}

/// Runs one command through the shell with inherited stdio, so its output appears live.
fn execute_single_command_interactive(
    command: &str,
    working_dir: &Path,
    command_number: usize,
) -> Result<()> {
    if command.trim().is_empty() {
        return Err(GraftError::command(format!("Command #{command_number} is empty")).into());
    }

    info!("Executing command #{}: {}", command_number, command);

    let (shell, shell_args) = get_shell_command();
    let mut cmd_args = shell_args;
    cmd_args.push(command.to_owned());

    let status = Command::new(&shell)
        .args(&cmd_args)
        .current_dir(working_dir)
        .status()
        .with_context(|| format!("Failed to execute command #{command_number}: {command}"))?;

    if !status.success() {
        return Err(GraftError::command(format!(
            "Command #{} failed with exit code {}: {}\nWorking directory: {}",
            command_number,
            status.code().unwrap_or(-1_i32),
            command,
            working_dir.display()
        ))
        .into());
    }

    Ok(())
}

/// Returns the shell and its run-a-string flag: `cmd /C` on Windows, `sh -c` elsewhere.
fn get_shell_command() -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        ("cmd".to_owned(), vec!["/C".to_owned()])
    } else {
        ("sh".to_owned(), vec!["-c".to_owned()])
    }
}

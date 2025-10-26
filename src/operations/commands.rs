//! Command execution with proper working directory context

use crate::error::GraftError;
use anyhow::{Context as _, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::info;

/// Execute a list of commands in the specified working directory
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

    let mut executed_count = 0;

    for (index, command) in commands.iter().enumerate() {
        execute_single_command(command, work_path, index + 1)?;
        executed_count += 1;
    }

    Ok(executed_count)
}

/// Execute a single command in the specified working directory
fn execute_single_command(command: &str, working_dir: &Path, command_number: usize) -> Result<()> {
    if command.trim().is_empty() {
        return Err(GraftError::command(format!("Command #{command_number} is empty")).into());
    }

    // Parse command into parts (shell, -c, command)
    let (shell, shell_args) = get_shell_command();
    let mut cmd_args = shell_args;
    cmd_args.push(command.to_owned());

    // Execute command
    let output = Command::new(&shell)
        .args(&cmd_args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute command #{command_number}: {command}"))?;

    // Check exit status
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut error_msg = format!(
            "Command #{} failed with exit code {}: {}\n",
            command_number,
            output.status.code().unwrap_or(-1),
            command
        );

        if !stderr.trim().is_empty() {
            error_msg.push_str(&format!("Error output:\n{}\n", stderr.trim()));
        }

        if !stdout.trim().is_empty() {
            error_msg.push_str(&format!("Standard output:\n{}\n", stdout.trim()));
        }

        error_msg.push_str(&format!("Working directory: {}", working_dir.display()));

        return Err(GraftError::command(error_msg).into());
    }

    // Print command output for visibility
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        info!("Command #{} output:", command_number);
        info!("{}", stdout.trim());
    }

    Ok(())
}

/// Get the appropriate shell command for the current platform
fn get_shell_command() -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        return ("cmd".to_owned(), vec!["/C".to_owned()]);
    } else {
        return ("sh".to_owned(), vec!["-c".to_owned()]);
    }
}

/// Execute commands with real-time output (for interactive commands)
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

    let mut executed_count = 0;

    for (index, command) in commands.iter().enumerate() {
        execute_single_command_interactive(command, work_path, index + 1)?;
        executed_count += 1;
    }

    Ok(executed_count)
}

/// Execute a single command with real-time output
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

    // Execute command with inherited stdio for real-time output
    let status = Command::new(&shell)
        .args(&cmd_args)
        .current_dir(working_dir)
        .status()
        .with_context(|| format!("Failed to execute command #{command_number}: {command}"))?;

    // Check exit status
    if !status.success() {
        return Err(GraftError::command(format!(
            "Command #{} failed with exit code {}: {}\nWorking directory: {}",
            command_number,
            status.code().unwrap_or(-1),
            command,
            working_dir.display()
        ))
        .into());
    }

    Ok(())
}

/// Validate commands before execution (for dry run)
pub fn validate_commands(commands: &[String]) -> Result<Vec<CommandValidation>> {
    let mut validations = Vec::new();

    for (index, command) in commands.iter().enumerate() {
        let validation = CommandValidation {
            command: command.clone(),
            command_number: index + 1,
            is_valid: !command.trim().is_empty(),
            potential_issues: analyze_command_safety(command),
        };

        validations.push(validation);
    }

    Ok(validations)
}

/// Analyze a command for potential security or safety issues
fn analyze_command_safety(command: &str) -> Vec<String> {
    let mut issues = Vec::new();

    // Check for potentially destructive commands
    let dangerous_patterns = [
        "rm -rf",
        "rm -f",
        "format",
        "mkfs",
        "dd if=",
        ":(){ :|:& };:", // Fork bomb
        "sudo rm",
    ];

    for pattern in &dangerous_patterns {
        if command.to_lowercase().contains(&pattern.to_lowercase()) {
            issues.push(format!(
                "Contains potentially destructive command: {pattern}"
            ));
        }
    }

    // Check for network access patterns
    let network_patterns = ["curl", "wget", "nc ", "netcat"];

    for pattern in &network_patterns {
        if command.to_lowercase().contains(&pattern.to_lowercase()) {
            issues.push("Command may access network resources".to_owned());
            break;
        }
    }

    // Check for script execution
    if command.contains("eval") || command.contains("exec") {
        issues.push("Command contains script execution which could be risky".to_owned());
    }

    issues
}

/// Information about command validation
#[derive(Debug)]
pub struct CommandValidation {
    pub command: String,
    pub command_number: usize,
    pub is_valid: bool,
    pub potential_issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_execute_simple_command() {
        let temp_dir = TempDir::new().unwrap();

        // Test a simple echo command
        let commands = vec!["echo 'test' > output.txt".to_string()];

        let result = execute_commands(&commands, temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Verify the output file was created
        let output_file = temp_dir.path().join("output.txt");
        assert!(output_file.exists());
    }

    #[test]
    fn test_command_validation() {
        let commands = vec![
            "echo hello".to_string(),
            "rm -rf /".to_string(),
            "curl http://example.com".to_string(),
        ];

        let validations = validate_commands(&commands).unwrap();

        assert_eq!(validations.len(), 3);
        assert!(validations[0].is_valid);
        assert!(validations[0].potential_issues.is_empty());

        assert!(validations[1].is_valid); // Valid syntax but dangerous
        assert!(!validations[1].potential_issues.is_empty());

        assert!(validations[2].is_valid);
        assert!(!validations[2].potential_issues.is_empty());
    }
}

//! Tests for `execute_commands`, `execute_commands_interactive` and `validate_commands`.
//!
//! They run real shell commands in a temporary directory and check what validation flags.

#![expect(clippy::unwrap_used, reason = "This is a test module")]
#![expect(
    clippy::indexing_slicing,
    reason = "test code uses indexing after length assertions"
)]
#![expect(
    clippy::shadow_unrelated,
    reason = "test functions reuse variable names for clarity"
)]

use os_shim::System as _;
use os_shim::real::RealSystem;
use pollster::FutureExt as _;
use tixgraft::operations::commands::{
    execute_commands, execute_commands_interactive, validate_commands,
};

#[test]
fn execute_simple_command() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();

    let commands = vec!["echo 'test' > output.txt".to_owned()];

    let result = execute_commands(&commands, temp_dir.path().to_str().unwrap());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    let output_file = temp_dir.path().join("output.txt");
    assert!(output_file.exists());
}

/// A risky command stays valid and carries its potential issues.
#[test]
fn command_validation() {
    let commands = vec![
        "echo hello".to_owned(),
        "rm -rf /".to_owned(),
        "curl http://example.com".to_owned(),
    ];

    let validations = validate_commands(&commands).unwrap();

    assert_eq!(validations.len(), 3);
    assert!(validations[0].is_valid);
    assert_eq!(validations[0].potential_issues, Vec::<String>::new());

    assert!(validations[1].is_valid);
    assert_ne!(validations[1].potential_issues, Vec::<String>::new());

    assert!(validations[2].is_valid);
    assert_ne!(validations[2].potential_issues, Vec::<String>::new());
}

#[test]
fn execute_commands_empty_list() {
    let result = execute_commands(&[], "/tmp");
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn execute_commands_nonexistent_dir() {
    let commands = vec!["echo test".to_owned()];
    execute_commands(&commands, "/nonexistent_dir_12345").unwrap_err();
}

#[test]
fn execute_commands_failing_command() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();

    let commands = vec!["exit 1".to_owned()];
    let result = execute_commands(&commands, temp_dir.path().to_str().unwrap());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("failed"));
}

#[test]
fn execute_commands_interactive_empty() {
    let result = execute_commands_interactive(&[], "/tmp");
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn execute_commands_interactive_nonexistent_dir() {
    let commands = vec!["echo test".to_owned()];
    execute_commands_interactive(&commands, "/nonexistent_dir_12345").unwrap_err();
}

#[test]
fn execute_commands_interactive_success() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();

    let commands = vec!["true".to_owned()];
    let result = execute_commands_interactive(&commands, temp_dir.path().to_str().unwrap());
    assert_eq!(result.unwrap(), 1);
}

#[test]
fn execute_commands_interactive_failure() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();

    let commands = vec!["false".to_owned()];
    execute_commands_interactive(&commands, temp_dir.path().to_str().unwrap()).unwrap_err();
}

#[test]
fn validate_commands_empty_command() {
    let commands = vec!["  ".to_owned()];
    let validations = validate_commands(&commands).unwrap();
    assert_eq!(validations.len(), 1);
    assert!(!validations[0].is_valid);
}

#[test]
fn validate_commands_destructive_patterns() {
    let validations = validate_commands(&["sudo rm -rf /important".to_owned()]).unwrap();
    assert_ne!(validations[0].potential_issues, Vec::<String>::new());

    let validations = validate_commands(&["dd if=/dev/zero of=/dev/sda".to_owned()]).unwrap();
    assert_ne!(validations[0].potential_issues, Vec::<String>::new());
}

#[test]
fn validate_commands_network_patterns() {
    let validations = validate_commands(&["wget http://example.com".to_owned()]).unwrap();
    assert!(
        validations[0]
            .potential_issues
            .iter()
            .any(|i| i.contains("network"))
    );
}

#[test]
fn validate_commands_eval_exec() {
    let validations = validate_commands(&["eval $(foo)".to_owned()]).unwrap();
    assert!(
        validations[0]
            .potential_issues
            .iter()
            .any(|i| i.contains("script execution"))
    );
}

#[test]
fn validate_commands_safe_command() {
    let validations = validate_commands(&["ls -la".to_owned()]).unwrap();
    assert!(validations[0].is_valid);
    assert_eq!(validations[0].potential_issues, Vec::<String>::new());
}

#[test]
fn execute_multiple_commands() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();

    let commands = vec![
        "echo 'first' > first.txt".to_owned(),
        "echo 'second' > second.txt".to_owned(),
    ];
    let result = execute_commands(&commands, temp_dir.path().to_str().unwrap());
    assert_eq!(result.unwrap(), 2);
    assert!(temp_dir.path().join("first.txt").exists());
    assert!(temp_dir.path().join("second.txt").exists());
}

//! Unit tests for command execution

use tixgraft::operations::commands::{execute_commands, validate_commands};
use tixgraft::system::{RealSystem, System};

#[test]
fn test_execute_simple_command() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();

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

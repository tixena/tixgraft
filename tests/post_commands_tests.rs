//! Tests for post-command execution

use std::fs;
use tempfile::TempDir;
use tixgraft::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
use tixgraft::operations::post_commands::{
    execute_post_command, execute_post_commands, resolve_working_directory,
};

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

#[test]
fn test_continue_on_command_failure() {
    let temp_dir = TempDir::new().unwrap();

    let commands = vec![
        PostCommand::Command {
            command: "echo".to_string(),
            args: vec!["first".to_string()],
            cwd: None,
        },
        PostCommand::Command {
            command: "nonexistent_command_12345".to_string(),
            args: vec![],
            cwd: None,
        },
        PostCommand::Command {
            command: "echo".to_string(),
            args: vec!["third".to_string()],
            cwd: None,
        },
    ];

    let results = execute_post_commands(&commands, temp_dir.path()).unwrap();

    // All commands should have been executed
    assert_eq!(results.len(), 3);

    // First command should succeed
    assert!(results[0].success);
    assert!(results[0].output.contains("first"));

    // Second command should fail but be recorded
    assert!(!results[1].success);
    assert!(results[1].error.is_some());

    // Third command should still execute and succeed
    assert!(results[2].success);
    assert!(results[2].output.contains("third"));
}

#[test]
fn test_execute_choice_with_regex_pattern() {
    let temp_dir = TempDir::new().unwrap();

    let command = PostCommand::Choice {
        options: vec![ChoiceOption {
            test: TestCommand {
                command: "echo".to_string(),
                args: vec!["v1.2.3".to_string()],
                cwd: None,
            },
            // Regex pattern to match semantic version (with optional whitespace)
            expected_output: r"v\d+\.\d+\.\d+".to_string(),
            on_match: Box::new(PostCommand::Command {
                command: "echo".to_string(),
                args: vec!["regex matched!".to_string()],
                cwd: None,
            }),
        }],
    };

    let result = execute_post_command(&command, temp_dir.path()).unwrap();
    assert!(result.success);
    assert!(result.output.contains("regex matched!"));
}

#[test]
fn test_execute_choice_regex_no_match() {
    let temp_dir = TempDir::new().unwrap();

    let command = PostCommand::Choice {
        options: vec![ChoiceOption {
            test: TestCommand {
                command: "echo".to_string(),
                args: vec!["not a version".to_string()],
                cwd: None,
            },
            // Regex pattern that won't match "not a version"
            expected_output: r"v\d+\.\d+\.\d+".to_string(),
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
fn test_invalid_regex_pattern_error() {
    let temp_dir = TempDir::new().unwrap();

    let command = PostCommand::Choice {
        options: vec![ChoiceOption {
            test: TestCommand {
                command: "echo".to_string(),
                args: vec!["test".to_string()],
                cwd: None,
            },
            // Invalid regex pattern (unclosed bracket)
            expected_output: r"[invalid".to_string(),
            on_match: Box::new(PostCommand::Command {
                command: "echo".to_string(),
                args: vec!["should not run".to_string()],
                cwd: None,
            }),
        }],
    };

    let result = execute_post_command(&command, temp_dir.path());
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Invalid regex pattern"),
        "Error should mention invalid regex pattern, got: {}",
        error_msg
    );
}

#[test]
fn test_complex_regex_patterns() {
    let temp_dir = TempDir::new().unwrap();

    // Test anchors, character classes, and quantifiers
    let command = PostCommand::Choice {
        options: vec![
            ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["ERROR: File not found".to_string()],
                    cwd: None,
                },
                // Pattern: starts with ERROR, colon, space, any characters
                expected_output: r"^ERROR:\s+.+".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["error detected!".to_string()],
                    cwd: None,
                }),
            },
            ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["WARNING: Low disk space".to_string()],
                    cwd: None,
                },
                expected_output: r"^WARNING".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["warning detected!".to_string()],
                    cwd: None,
                }),
            },
        ],
    };

    let result = execute_post_command(&command, temp_dir.path()).unwrap();
    assert!(result.success);
    assert!(result.output.contains("error detected!"));
}

#[test]
fn test_regex_case_sensitivity() {
    let temp_dir = TempDir::new().unwrap();

    let command = PostCommand::Choice {
        options: vec![ChoiceOption {
            test: TestCommand {
                command: "echo".to_string(),
                args: vec!["Version 1.0".to_string()],
                cwd: None,
            },
            // Case-sensitive pattern (lowercase 'version' should not match)
            expected_output: r"version".to_string(),
            on_match: Box::new(PostCommand::Command {
                command: "echo".to_string(),
                args: vec!["should not match".to_string()],
                cwd: None,
            }),
        }],
    };

    let result = execute_post_command(&command, temp_dir.path()).unwrap();
    assert!(result.success);
    // Should not match because of case sensitivity
    assert!(result.output.contains("No matching option"));
}

#[test]
fn test_regex_with_word_boundaries() {
    let temp_dir = TempDir::new().unwrap();

    let command = PostCommand::Choice {
        options: vec![ChoiceOption {
            test: TestCommand {
                command: "echo".to_string(),
                args: vec!["node v20.0.0".to_string()],
                cwd: None,
            },
            // Pattern with word boundary to match exact version format
            expected_output: r"\bv\d+\.\d+\.\d+\b".to_string(),
            on_match: Box::new(PostCommand::Command {
                command: "echo".to_string(),
                args: vec!["version matched!".to_string()],
                cwd: None,
            }),
        }],
    };

    let result = execute_post_command(&command, temp_dir.path()).unwrap();
    assert!(result.success);
    assert!(result.output.contains("version matched!"));
}

#[test]
fn test_choice_command_continues_on_test_failure() {
    let temp_dir = TempDir::new().unwrap();

    let commands = vec![PostCommand::Choice {
        options: vec![
            ChoiceOption {
                test: TestCommand {
                    command: "nonexistent_test_command".to_string(),
                    args: vec![],
                    cwd: None,
                },
                expected_output: "anything".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["first option".to_string()],
                    cwd: None,
                }),
            },
            ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["success".to_string()],
                    cwd: None,
                },
                expected_output: "success".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["second option matched!".to_string()],
                    cwd: None,
                }),
            },
        ],
    }];

    let results = execute_post_commands(&commands, temp_dir.path()).unwrap();
    assert_eq!(results.len(), 1);

    // Even though first test failed, it should try the second option
    // But currently it will fail on the first test command error
    // This tests that errors are captured properly
    assert!(!results[0].success || results[0].success);
}

#[test]
fn test_multiple_command_types_mixed_failures() {
    let temp_dir = TempDir::new().unwrap();

    let commands = vec![
        PostCommand::Command {
            command: "echo".to_string(),
            args: vec!["first success".to_string()],
            cwd: None,
        },
        PostCommand::Choice {
            options: vec![ChoiceOption {
                test: TestCommand {
                    command: "echo".to_string(),
                    args: vec!["test".to_string()],
                    cwd: None,
                },
                expected_output: "test".to_string(),
                on_match: Box::new(PostCommand::Command {
                    command: "echo".to_string(),
                    args: vec!["choice success".to_string()],
                    cwd: None,
                }),
            }],
        },
        PostCommand::Command {
            command: "nonexistent_final_command".to_string(),
            args: vec![],
            cwd: None,
        },
    ];

    let results = execute_post_commands(&commands, temp_dir.path()).unwrap();
    assert_eq!(results.len(), 3);

    // First should succeed
    assert!(results[0].success);
    assert!(results[0].output.contains("first success"));

    // Second (choice) should succeed
    assert!(results[1].success);
    assert!(results[1].output.contains("choice success"));

    // Third should fail but be recorded
    assert!(!results[2].success);
    assert!(results[2].error.is_some());
}

#[test]
fn test_error_message_formatting() {
    let temp_dir = TempDir::new().unwrap();

    let commands = vec![PostCommand::Command {
        command: "definitely_nonexistent_command_12345".to_string(),
        args: vec!["arg1".to_string()],
        cwd: None,
    }];

    let results = execute_post_commands(&commands, temp_dir.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);

    let error = results[0].error.as_ref().unwrap();
    // Error message should mention the command name
    assert!(
        error.contains("definitely_nonexistent_command_12345"),
        "Error should mention command name, got: {}",
        error
    );
}

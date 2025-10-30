//! Tests for post-command execution

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use std::fs;
    use tempfile::TempDir;
    use tixgraft::config::graft_yaml::{ChoiceOption, PostCommand, TestCommand};
    use tixgraft::operations::post_commands::{
        execute_post_command, execute_post_commands, resolve_working_directory,
    };

    #[test]
    fn execute_simple_command() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Command {
            command: "echo".to_owned(),
            args: vec!["Hello, World!".to_owned()],
            cwd: None,
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("Hello, World!"));
    }

    #[test]
    fn execute_command_with_cwd() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Create a file in subdir to verify cwd
        fs::write(sub_dir.join("test.txt"), "content").unwrap();

        let command = PostCommand::new("ls".to_owned(), vec![], Some("subdir".to_owned()));

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
    }

    #[test]
    fn execute_choice_with_match() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["version 1.0".to_owned()], None),
                "version".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["matched!".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("matched!"));
    }

    #[test]
    fn execute_choice_no_match() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["hello".to_owned()], None),
                "version".to_owned(), // Won't match "hello"
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["should not run".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matching option"));
    }

    #[test]
    fn execute_nested_choice() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["outer".to_owned()], None),
                "outer".to_owned(),
                Box::new(PostCommand::Choice {
                    options: vec![ChoiceOption::new(
                        TestCommand::new("echo".to_owned(), vec!["inner".to_owned()], None),
                        "inner".to_owned(),
                        Box::new(PostCommand::new(
                            "echo".to_owned(),
                            vec!["nested match!".to_owned()],
                            None,
                        )),
                    )],
                }),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("nested match!"));
    }

    #[test]
    fn resolve_working_directory_tst() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Test None (uses graft_directory)
        let result = resolve_working_directory(None, temp_dir.path()).unwrap();
        assert_eq!(result, temp_dir.path());

        // Test relative path
        let result_2 = resolve_working_directory(Some("subdir"), temp_dir.path()).unwrap();
        assert_eq!(result_2, sub_dir);

        // Test non-existent directory
        let result_3 = resolve_working_directory(Some("nonexistent"), temp_dir.path());
        result_3.unwrap_err();
    }

    #[test]
    fn continue_on_command_failure() {
        let temp_dir = TempDir::new().unwrap();

        let commands = vec![
            PostCommand::new("echo".to_owned(), vec!["first".to_owned()], None),
            PostCommand::new("nonexistent_command_12345".to_owned(), vec![], None),
            PostCommand::new("echo".to_owned(), vec!["third".to_owned()], None),
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
    fn execute_choice_with_regex_pattern() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["v1.2.3".to_owned()], None),
                // Regex pattern to match semantic version (with optional whitespace)
                r"v\d+\.\d+\.\d+".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["regex matched!".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("regex matched!"));
    }

    #[test]
    fn execute_choice_regex_no_match() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["not a version".to_owned()], None),
                // Regex pattern that won't match "not a version"
                r"v\d+\.\d+\.\d+".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["should not run".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("No matching option"));
    }

    #[test]
    fn invalid_regex_pattern_error() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["test".to_owned()], None),
                // Invalid regex pattern (unclosed bracket)
                "[invalid".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["should not run".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path());
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid regex pattern"),
            "Error should mention invalid regex pattern, got: {error_msg}"
        );
    }

    #[test]
    fn complex_regex_patterns() {
        let temp_dir = TempDir::new().unwrap();

        // Test anchors, character classes, and quantifiers
        let command = PostCommand::Choice {
            options: vec![
                ChoiceOption::new(
                    TestCommand::new(
                        "echo".to_owned(),
                        vec!["ERROR: File not found".to_owned()],
                        None,
                    ),
                    // Pattern: starts with ERROR, colon, space, any characters
                    r"^ERROR:\s+.+".to_owned(),
                    Box::new(PostCommand::new(
                        "echo".to_owned(),
                        vec!["error detected!".to_owned()],
                        None,
                    )),
                ),
                ChoiceOption::new(
                    TestCommand::new(
                        "echo".to_owned(),
                        vec!["WARNING: Low disk space".to_owned()],
                        None,
                    ),
                    "^WARNING".to_owned(),
                    Box::new(PostCommand::new(
                        "echo".to_owned(),
                        vec!["warning detected!".to_owned()],
                        None,
                    )),
                ),
            ],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("error detected!"));
    }

    #[test]
    fn regex_case_sensitivity() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["Version 1.0".to_owned()], None),
                // Case-sensitive pattern (lowercase 'version' should not match)
                "version".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["should not match".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        // Should not match because of case sensitivity
        assert!(result.output.contains("No matching option"));
    }

    #[test]
    fn regex_with_word_boundaries() {
        let temp_dir = TempDir::new().unwrap();

        let command = PostCommand::Choice {
            options: vec![ChoiceOption::new(
                TestCommand::new("echo".to_owned(), vec!["node v20.0.0".to_owned()], None),
                // Pattern with word boundary to match exact version format
                r"\bv\d+\.\d+\.\d+\b".to_owned(),
                Box::new(PostCommand::new(
                    "echo".to_owned(),
                    vec!["version matched!".to_owned()],
                    None,
                )),
            )],
        };

        let result = execute_post_command(&command, temp_dir.path()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("version matched!"));
    }

    #[test]
    fn choice_command_continues_on_test_failure() {
        let temp_dir = TempDir::new().unwrap();

        let commands = vec![PostCommand::Choice {
            options: vec![
                ChoiceOption::new(
                    TestCommand::new("nonexistent_test_command".to_owned(), vec![], None),
                    "anything".to_owned(),
                    Box::new(PostCommand::new(
                        "echo".to_owned(),
                        vec!["first option".to_owned()],
                        None,
                    )),
                ),
                ChoiceOption::new(
                    TestCommand::new("echo".to_owned(), vec!["success".to_owned()], None),
                    "success".to_owned(),
                    Box::new(PostCommand::new(
                        "echo".to_owned(),
                        vec!["second option matched!".to_owned()],
                        None,
                    )),
                ),
            ],
        }];

        let results = execute_post_commands(&commands, temp_dir.path()).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn multiple_command_types_mixed_failures() {
        let temp_dir = TempDir::new().unwrap();

        let commands = vec![
            PostCommand::new("echo".to_owned(), vec!["first success".to_owned()], None),
            PostCommand::Choice {
                options: vec![ChoiceOption::new(
                    TestCommand::new("echo".to_owned(), vec!["test".to_owned()], None),
                    "test".to_owned(),
                    Box::new(PostCommand::new(
                        "echo".to_owned(),
                        vec!["choice success".to_owned()],
                        None,
                    )),
                )],
            },
            PostCommand::new("nonexistent_final_command".to_owned(), vec![], None),
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
    fn error_message_formatting() {
        let temp_dir = TempDir::new().unwrap();

        let commands = vec![PostCommand::new(
            "definitely_nonexistent_command_12345".to_owned(),
            vec!["arg1".to_owned()],
            None,
        )];

        let results = execute_post_commands(&commands, temp_dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);

        let error = results[0].error.as_ref().unwrap();
        // Error message should mention the command name
        assert!(
            error.contains("definitely_nonexistent_command_12345"),
            "Error should mention command name, got: {error}"
        );
    }
}

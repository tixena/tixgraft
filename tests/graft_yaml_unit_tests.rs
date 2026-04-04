//! Unit tests for `graft_yaml` public API.
//!
//! Private function tests remain inline in `src/config/graft_yaml.rs`.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
#[expect(
    clippy::indexing_slicing,
    reason = "test code uses indexing after length assertions"
)]
mod tests {
    use os_shim::mock::MockSystem;
    use os_shim::real::RealSystem;
    use std::io::Write as _;
    use std::path::Path;
    use tempfile::NamedTempFile;
    use tixgraft::config::graft_yaml::{
        ChoiceOption, GraftConfig, GraftReplacement, PostCommand, TestCommand,
    };

    #[test]
    fn load_valid_graft_config() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
context:
  - name: projectName
    description: The project name
    dataType: string

  - name: maxGbPerPod
    description: Max GB per pod
    dataType: number
    defaultValue: 10

replacements:
  - source: "{{PROJECT_NAME}}"
    valueFromContext: projectName

  - source: "{{MAX_GB}}"
    valueFromContext: maxGbPerPod

postCommands:
  - command: echo
    args: ["Hello"]
"#
        )
        .unwrap();

        let system = RealSystem::new();
        let result = GraftConfig::load_from_file(&system, temp_file.path());
        let config = result.unwrap();
        assert_eq!(config.context.len(), 2);
        assert_eq!(config.replacements.len(), 2);
        assert_eq!(config.post_commands.len(), 1);
    }

    #[test]
    fn validate_replacement_exclusivity() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
replacements:
  - source: "{{VAR}}"
    target: "value"
    valueFromEnv: "ENV_VAR"
"#
        )
        .unwrap();

        let system = RealSystem::new();
        let result = GraftConfig::load_from_file(&system, temp_file.path());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must specify exactly one of"));
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "panic used as test assertion for enum variant mismatch"
    )]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "match on borrowed enum in test code"
    )]
    fn post_command_default_type() {
        let yaml = r#"
postCommands:
  - command: npm
    args: ["install"]
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.post_commands.len(), 1);
        match &config.post_commands[0] {
            PostCommand::Command { command, .. } => {
                assert_eq!(command, "npm");
            }
            PostCommand::Choice { .. } | _ => panic!("Expected Command type"),
        }
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "panic used as test assertion for enum variant mismatch"
    )]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "match on borrowed enum in test code"
    )]
    fn post_command_choice_type() {
        let yaml = r#"
postCommands:
  - type: choice
    options:
      - test:
          command: node
          args: ["--version"]
        expectedOutput: "v"
        onMatch:
          command: npm
          args: ["install"]
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.post_commands.len(), 1);
        match &config.post_commands[0] {
            PostCommand::Choice { options } => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].test.command, "node");
                assert_eq!(options[0].expected_output, "v");
            }
            PostCommand::Command { .. } | _ => panic!("Expected Choice type"),
        }
    }

    #[test]
    fn load_from_file_not_found() {
        let system = MockSystem::new();
        let err = GraftConfig::load_from_file(&system, Path::new("/nonexistent/.graft.yaml"))
            .unwrap_err()
            .to_string();
        assert!(err.contains(".graft.yaml not found"));
    }

    #[test]
    fn load_from_file_with_mock() {
        let yaml_content = r#"
context:
  - name: test
    description: Test variable
    dataType: string
replacements:
  - source: "{{TEST}}"
    target: "value"
"#;

        let system = MockSystem::new()
            .with_file("/test/.graft.yaml", yaml_content.as_bytes())
            .unwrap();

        let config = GraftConfig::load_from_file(&system, Path::new("/test/.graft.yaml")).unwrap();
        assert_eq!(config.context.len(), 1);
        assert_eq!(config.context[0].name, "test");
        assert_eq!(config.replacements.len(), 1);
    }

    #[test]
    fn load_from_string_invalid_yaml() {
        GraftConfig::load_from_string("not: [valid: yaml: {").unwrap_err();
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "panic used as test assertion for enum variant mismatch"
    )]
    fn post_command_default_impl() {
        let cmd = PostCommand::default();
        match cmd {
            PostCommand::Command {
                command, args, cwd, ..
            } => {
                assert!(command.is_empty());
                assert!(args.is_empty());
                assert!(cwd.is_none());
            }
            PostCommand::Choice { .. } | _ => panic!("Expected Command default"),
        }
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "panic used as test assertion for enum variant mismatch"
    )]
    fn post_command_new_tst() {
        let cmd = PostCommand::new(
            "echo".to_owned(),
            vec!["hello".to_owned()],
            Some("/tmp".to_owned()),
        );
        match cmd {
            PostCommand::Command {
                command, args, cwd, ..
            } => {
                assert_eq!(command, "echo");
                assert_eq!(args, vec!["hello"]);
                assert_eq!(cwd, Some("/tmp".to_owned()));
            }
            PostCommand::Choice { .. } | _ => panic!("Expected Command"),
        }
    }

    #[test]
    #[expect(
        clippy::panic,
        reason = "panic used as test assertion for enum variant mismatch"
    )]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "match on borrowed enum in test code"
    )]
    fn post_command_explicit_command_type() {
        let yaml = "postCommands:\n  - type: command\n    command: npm\n    args: [\"install\"]\n    cwd: \"/app\"\n";
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.post_commands.len(), 1);
        match &config.post_commands[0] {
            PostCommand::Command {
                command, args, cwd, ..
            } => {
                assert_eq!(command, "npm");
                assert_eq!(args, &vec!["install".to_owned()]);
                assert_eq!(cwd, &Some("/app".to_owned()));
            }
            PostCommand::Choice { .. } | _ => panic!("Expected Command type"),
        }
    }

    #[test]
    fn post_command_unknown_type() {
        let yaml = "postCommands:\n  - type: invalid\n    command: foo\n";
        let result: Result<GraftConfig, _> = serde_yaml::from_str(yaml);
        result.unwrap_err();
    }

    #[test]
    fn graft_replacement_new_with_env() {
        let repl =
            GraftReplacement::new("{{VAR}}".to_owned(), None, Some("MY_ENV".to_owned()), None);
        assert_eq!(repl.source, "{{VAR}}");
        assert!(repl.target.is_none());
        assert_eq!(repl.value_from_env, Some("MY_ENV".to_owned()));
        assert!(repl.value_from_context.is_none());
    }

    #[test]
    fn create_test_command() {
        let cmd = TestCommand::new(
            "node".to_owned(),
            vec!["--version".to_owned()],
            Some("/app".to_owned()),
        );
        assert_eq!(cmd.command, "node");
        assert_eq!(cmd.args, vec!["--version"]);
        assert_eq!(cmd.cwd, Some("/app".to_owned()));
    }

    #[test]
    fn choice_option_new_tst() {
        let test_cmd = TestCommand::new("test".to_owned(), vec![], None);
        let on_match = Box::new(PostCommand::new("echo".to_owned(), vec![], None));
        let option = ChoiceOption::new(test_cmd, "expected".to_owned(), on_match);
        assert_eq!(option.expected_output, "expected");
        assert_eq!(option.test.command, "test");
    }
}

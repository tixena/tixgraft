#![expect(clippy::unwrap_used, reason = "These are unit tests")]
#![expect(
    clippy::indexing_slicing,
    reason = "index-based assertions are acceptable in tests"
)]

use std::collections::HashMap;

use super::*;

#[test]
fn shell_escape_simple() {
    assert_eq!(shell_escape("simple"), "simple");
    assert_eq!(shell_escape("path/to/file"), "path/to/file");
    assert_eq!(shell_escape("file.txt"), "file.txt");
    assert_eq!(shell_escape("repo-name"), "repo-name");
}

#[test]
fn shell_escape_special_chars() {
    assert_eq!(shell_escape("has space"), r#""has space""#);
    assert_eq!(shell_escape("has$dollar"), r#""has\$dollar""#);
    assert_eq!(shell_escape(r#"has"quote"#), r#""has\"quote""#);
    assert_eq!(shell_escape("back\\slash"), r#""back\\slash""#);
}

#[test]
fn format_replacement_tst() {
    let repl_static = ReplacementConfig {
        source: "{{VAR}}".to_owned(),
        target: Some("value".to_owned()),
        value_from_env: None,
    };
    assert_eq!(format_replacement(&repl_static), "{{VAR}}=value");

    let repl_env = ReplacementConfig {
        source: "{{VAR}}".to_owned(),
        target: None,
        value_from_env: Some("MY_ENV".to_owned()),
    };
    assert_eq!(format_replacement(&repl_env), "{{VAR}}=env:MY_ENV");
}

#[test]
fn output_format_from_str() {
    assert_eq!(
        "shell".parse::<OutputFormat>().unwrap(),
        OutputFormat::Shell
    );
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "SHELL".parse::<OutputFormat>().unwrap(),
        OutputFormat::Shell
    );
    "invalid".parse::<OutputFormat>().unwrap_err();
}

#[test]
fn build_command_args_basic() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: Some("main".to_owned()),
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    assert_eq!(args[0], "tixgraft");
    assert!(args.contains(&"--repository".to_owned()));
    assert!(args.contains(&"my_organization/repo".to_owned()));
    assert!(args.contains(&"--tag".to_owned()));
    assert!(args.contains(&"main".to_owned()));
    assert!(args.contains(&"--pull-source".to_owned()));
    assert!(args.contains(&"src".to_owned()));
    assert!(args.contains(&"--pull-target".to_owned()));
    assert!(args.contains(&"dst".to_owned()));
}

#[test]
fn build_command_args_with_reset() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: true,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    assert!(args.contains(&"--pull-reset".to_owned()));
}

#[test]
fn build_command_args_with_replacements() {
    use crate::cli::{PullConfig, ReplacementConfig};

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![
                ReplacementConfig {
                    source: "{{VAR1}}".to_owned(),
                    target: Some("value1".to_owned()),
                    value_from_env: None,
                },
                ReplacementConfig {
                    source: "{{VAR2}}".to_owned(),
                    target: None,
                    value_from_env: Some("MY_ENV".to_owned()),
                },
            ],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    assert!(args.contains(&"--pull-replacement".to_owned()));
    assert!(args.contains(&"{{VAR1}}=value1".to_owned()));
    assert!(args.contains(&"{{VAR2}}=env:MY_ENV".to_owned()));
}

#[test]
fn multiline_command() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec!["echo 'line1'\necho 'line2'".to_owned()],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let result = generate_command_line(&config, OutputFormat::Shell);
    assert!(result.is_ok());
    let output = result.unwrap();
    // Should escape the newline properly
    assert!(output.contains("--pull-commands"));
}

#[test]
fn replacement_with_special_chars() {
    use crate::cli::ReplacementConfig;

    let replacement = ReplacementConfig {
        source: "{{VAR}}".to_owned(),
        target: Some(r#"value with "quotes" and $vars"#.to_owned()),
        value_from_env: None,
    };

    let formatted = format_replacement(&replacement);
    assert_eq!(formatted, r#"{{VAR}}=value with "quotes" and $vars"#);

    // Now test that shell_escape properly escapes it
    let escaped = shell_escape(&formatted);
    assert!(escaped.contains(r#"\""#)); // Quotes should be escaped
    assert!(escaped.contains(r"\$")); // Dollar signs should be escaped
}

#[test]
fn replacement_with_newlines() {
    use crate::cli::ReplacementConfig;

    let replacement = ReplacementConfig {
        source: "{{VAR}}".to_owned(),
        target: Some("line1\nline2".to_owned()),
        value_from_env: None,
    };

    let formatted = format_replacement(&replacement);
    assert_eq!(formatted, "{{VAR}}=line1\nline2");

    // Shell escape should handle newlines
    let escaped = shell_escape(&formatted);
    assert!(escaped.starts_with('"'));
    assert!(escaped.ends_with('"'));
}

#[test]
fn empty_pulls_array() {
    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: Some("main".to_owned()),
        context: HashMap::new(),
        pulls: vec![],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    // Should still work, just no pull args
    assert_eq!(args[0], "tixgraft");
    assert!(args.contains(&"--repository".to_owned()));
    assert!(args.contains(&"my_organization/repo".to_owned()));
}

#[test]
fn path_with_spaces() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src with spaces".to_owned(),
            target: "dst with spaces".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let result = generate_command_line(&config, OutputFormat::Shell);
    assert!(result.is_ok());
    let output = result.unwrap();
    // Paths with spaces should be quoted
    assert!(output.contains(r#""src with spaces""#));
    assert!(output.contains(r#""dst with spaces""#));
}

#[test]
fn file_type_pull() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "file.txt".to_owned(),
            target: "output.txt".to_owned(),
            pull_type: "file".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    // File type should be included since it's not the default
    assert!(args.contains(&"--pull-type".to_owned()));
    assert!(args.contains(&"file".to_owned()));
}

#[test]
fn per_pull_overrides() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("global/repo".to_owned()),
        tag: Some("v1".to_owned()),
        context: HashMap::new(),
        pulls: vec![
            PullConfig {
                source: "src1".to_owned(),
                target: "dst1".to_owned(),
                pull_type: "directory".to_owned(),
                repository: None, // Uses global
                tag: None,        // Uses global
                reset: false,
                require_clean_target: true,
                must_succeed: true,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            },
            PullConfig {
                source: "src2".to_owned(),
                target: "dst2".to_owned(),
                pull_type: "directory".to_owned(),
                repository: Some("per-pull/repo".to_owned()), // Override
                tag: Some("v2".to_owned()),                   // Override
                reset: false,
                require_clean_target: true,
                must_succeed: true,
                commands: vec![],
                replacements: vec![],
                context: HashMap::new(),
            },
        ],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    // Should have per-pull overrides for the second pull
    assert!(args.contains(&"--pull-repository".to_owned()));
    assert!(args.contains(&"per-pull/repo".to_owned()));
    assert!(args.contains(&"--pull-tag".to_owned()));
    assert!(args.contains(&"v2".to_owned()));
}

#[test]
fn must_succeed_true_not_emitted() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: true,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    // Default must_succeed=true should NOT emit the flag
    assert!(!args.contains(&"--pull-must-succeed".to_owned()));
}

#[test]
fn must_succeed_false_emitted() {
    use crate::cli::PullConfig;

    let config = Config {
        repository: Some("my_organization/repo".to_owned()),
        tag: None,
        context: HashMap::new(),
        pulls: vec![PullConfig {
            source: "src".to_owned(),
            target: "dst".to_owned(),
            pull_type: "directory".to_owned(),
            repository: None,
            tag: None,
            reset: false,
            require_clean_target: true,
            must_succeed: false,
            commands: vec![],
            replacements: vec![],
            context: HashMap::new(),
        }],
        children: Vec::new(),
        process_children_first: false,
    };

    let args = build_command_args(&config);
    // must_succeed=false should emit the flag
    assert!(args.contains(&"--pull-must-succeed".to_owned()));
    assert!(args.contains(&"false".to_owned()));
}

//! Unit tests for children config parsing and validation.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
#[expect(
    clippy::indexing_slicing,
    reason = "Index-based assertions are acceptable in tests"
)]
mod tests {
    use os_shim::mock::MockSystem;
    use tixgraft::config::Config;
    use tixgraft::config::validation::validate_config;

    /// Helper to parse a YAML string into a Config.
    fn parse_config(yaml: &str) -> Config {
        serde_yaml::from_str(yaml).unwrap()
    }

    // ── Config parsing tests ────────────────────────────────────────────

    #[test]
    fn parse_children_only() {
        let config = parse_config(
            r#"
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        assert_eq!(config.children.len(), 1);
        assert_eq!(config.children[0], "./sub/tixgraft.yaml");
        assert!(config.pulls.is_empty());
        assert!(!config.process_children_first);
    }

    #[test]
    fn parse_pulls_and_children() {
        let config = parse_config(
            r#"
repository: "my_org/repo"
children:
  - "./sub/tixgraft.yaml"
pulls:
  - source: "src"
    target: "dst"
"#,
        );

        assert_eq!(config.children.len(), 1);
        assert_eq!(config.pulls.len(), 1);
        assert_eq!(config.pulls[0].source, "src");
    }

    #[test]
    fn parse_process_children_first_true() {
        let config = parse_config(
            r#"
processChildrenFirst: true
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        assert!(config.process_children_first);
    }

    #[test]
    fn parse_process_children_first_default() {
        let config = parse_config(
            r#"
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        assert!(!config.process_children_first);
    }

    #[test]
    fn parse_process_children_first_false() {
        let config = parse_config(
            r#"
processChildrenFirst: false
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        assert!(!config.process_children_first);
    }

    #[test]
    fn serialize_omits_empty_children() {
        let config = parse_config(
            r#"
repository: "my_org/repo"
pulls:
  - source: "src"
    target: "dst"
"#,
        );

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(!yaml.contains("children"));
        assert!(!yaml.contains("processChildrenFirst"));
    }

    #[test]
    fn serialize_includes_nonempty_children() {
        let config = parse_config(
            r#"
processChildrenFirst: true
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("children"));
        assert!(yaml.contains("./sub/tixgraft.yaml"));
        assert!(yaml.contains("processChildrenFirst"));
    }

    #[test]
    fn backward_compat_existing_config() {
        let config = parse_config(
            r#"
repository: "my_organization/scaffolds"
tag: "main"
pulls:
  - source: "kubernetes/mongodb"
    target: "./k8s/mongodb"
    type: "directory"
"#,
        );

        assert_eq!(
            config.repository.as_deref(),
            Some("my_organization/scaffolds")
        );
        assert_eq!(config.tag.as_deref(), Some("main"));
        assert_eq!(config.pulls.len(), 1);
        assert!(config.children.is_empty());
        assert!(!config.process_children_first);
    }

    #[test]
    fn parse_multiple_children() {
        let config = parse_config(
            r#"
children:
  - "./services/api/tixgraft.yaml"
  - "./services/web/tixgraft.yaml"
  - "./infra/tixgraft.yaml"
"#,
        );

        assert_eq!(config.children.len(), 3);
        assert_eq!(config.children[0], "./services/api/tixgraft.yaml");
        assert_eq!(config.children[1], "./services/web/tixgraft.yaml");
        assert_eq!(config.children[2], "./infra/tixgraft.yaml");
    }

    // ── Validation tests ────────────────────────────────────────────────

    #[test]
    fn validate_pulls_only() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
repository: "my_org/repo"
pulls:
  - source: "src"
    target: "dst"
"#,
        );

        validate_config(&system, &config).unwrap();
    }

    #[test]
    fn validate_children_only() {
        let system = MockSystem::new()
            .with_file(
                "./sub/tixgraft.yaml",
                b"pulls:\n  - source: x\n    target: y\n",
            )
            .unwrap();

        let config = parse_config(
            r#"
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        validate_config(&system, &config).unwrap();
    }

    #[test]
    fn validate_both_pulls_and_children() {
        let system = MockSystem::new()
            .with_file(
                "./sub/tixgraft.yaml",
                b"pulls:\n  - source: x\n    target: y\n",
            )
            .unwrap();

        let config = parse_config(
            r#"
repository: "my_org/repo"
children:
  - "./sub/tixgraft.yaml"
pulls:
  - source: "src"
    target: "dst"
"#,
        );

        validate_config(&system, &config).unwrap();
    }

    #[test]
    fn validate_neither_fails() {
        let system = MockSystem::new();
        let config = parse_config("{}");

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("at least one"),
            "Expected 'at least one' in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_dotdot() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
children:
  - "../escape/tixgraft.yaml"
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("cannot contain '..'"),
            "Expected \"cannot contain '..'\" in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_absolute() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
children:
  - "/etc/tixgraft.yaml"
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("cannot be absolute"),
            "Expected 'cannot be absolute' in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_missing() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
children:
  - "./nonexistent.yaml"
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "Expected 'not found' in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_empty() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
children:
  - ""
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("cannot be empty"),
            "Expected 'cannot be empty' in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_whitespace_only() {
        let system = MockSystem::new();
        let config = parse_config(
            r#"
children:
  - "   "
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("cannot be empty"),
            "Expected 'cannot be empty' in error, got: {err}",
        );
    }

    #[test]
    fn validate_child_path_valid() {
        let system = MockSystem::new()
            .with_file(
                "./sub/tixgraft.yaml",
                b"children:\n  - ./nested/tixgraft.yaml\n",
            )
            .unwrap();

        let config = parse_config(
            r#"
children:
  - "./sub/tixgraft.yaml"
"#,
        );

        validate_config(&system, &config).unwrap();
    }

    #[test]
    fn validate_child_error_includes_index() {
        let system = MockSystem::new()
            .with_file(
                "./good/tixgraft.yaml",
                b"pulls:\n  - source: x\n    target: y\n",
            )
            .unwrap();

        let config = parse_config(
            r#"
children:
  - "./good/tixgraft.yaml"
  - "../bad/tixgraft.yaml"
"#,
        );

        let err = validate_config(&system, &config).unwrap_err();
        assert!(
            err.to_string().contains("Child config #2"),
            "Expected 'Child config #2' in error, got: {err}",
        );
    }
}

//! Configuration parsing and validation tests

use tixgraft::config::Config;
use tixgraft::system::MockSystem;

#[test]
fn test_valid_basic_config() {
    let config_content = r#"
repository: "my_organization/templates"
tag: "main"
pulls:
  - source: "docker/nodejs"
    target: "./docker"
    type: "directory"
"#;

    let system = MockSystem::new().with_file("/test/valid.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/valid.yaml");
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.repository.unwrap(), "my_organization/templates");
    assert_eq!(config.tag.unwrap(), "main");
    assert_eq!(config.pulls.len(), 1);
    assert_eq!(config.pulls[0].source, "docker/nodejs");
    assert_eq!(config.pulls[0].target, "./docker");
    assert_eq!(config.pulls[0].pull_type, "directory");
}

#[test]
fn test_config_with_replacements() {
    let config_content = r#"
repository: "my_organization/templates"
pulls:
  - source: "app/service"
    target: "./service"
    replacements:
      - source: "{{NAME}}"
        target: "my-service"
      - source: "{{VERSION}}"
        valueFromEnv: "TEST_ENV_VAR"
"#;

    let system = MockSystem::new()
        .with_env("TEST_ENV_VAR", "test_value")
        .with_file("/test/replacements.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/replacements.yaml");
    assert!(config.is_ok());

    let config = config.unwrap();
    assert_eq!(config.pulls[0].replacements.len(), 2);
    assert_eq!(config.pulls[0].replacements[0].source, "{{NAME}}");
    assert_eq!(
        config.pulls[0].replacements[0].target.as_ref().unwrap(),
        "my-service"
    );
    assert_eq!(
        config.pulls[0].replacements[1]
            .value_from_env
            .as_ref()
            .unwrap(),
        "TEST_ENV_VAR"
    );
}

#[test]
fn test_config_validation_invalid_schema() {
    let config_content = r#"
repository: "my_organization/templates"
pulls:
  - source: "docker/nodejs"
    target: "./docker"
    type: "invalid_type"  # Should be "file" or "directory"
"#;

    let system =
        MockSystem::new().with_file("/test/invalid_schema.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/invalid_schema.yaml");
    assert!(config.is_err());

    let error = config.unwrap_err();
    assert!(error.to_string().contains("validation failed"));
}

#[test]
fn test_config_validation_missing_env_var() {
    let config_content = r#"
repository: "my_organization/templates"
pulls:
  - source: "app/service"
    target: "./service"
    replacements:
      - source: "{{VERSION}}"
        valueFromEnv: "NONEXISTENT_VAR"
"#;

    let system = MockSystem::new().with_file("/test/missing_env.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/missing_env.yaml");
    assert!(config.is_err());

    let error = config.unwrap_err();
    assert!(error.to_string().contains("validation failed"));
}

#[test]
fn test_minimal_config() {
    let config_content = r#"
pulls:
  - source: "src"
    target: "./dest"
"#;

    let system = MockSystem::new().with_file("/test/minimal.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/minimal.yaml");
    assert!(config.is_ok());

    let config = config.unwrap();
    assert!(config.repository.is_none());
    assert!(config.tag.is_none());
    assert_eq!(config.pulls.len(), 1);
}

#[test]
fn test_config_directory_traversal_protection() {
    let config_content = r#"
repository: "my_organization/templates"
pulls:
  - source: "app"
    target: "../../etc/passwd"  # Path traversal attempt
"#;

    let system = MockSystem::new().with_file("/test/unsafe.yaml", config_content.as_bytes());

    let config = Config::load_from_file(&system, "/test/unsafe.yaml");
    assert!(config.is_err());

    let error = config.unwrap_err();
    assert!(error.to_string().contains("validation failed"));
}

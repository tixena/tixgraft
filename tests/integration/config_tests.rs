//! Configuration parsing and validation tests

use tixgraft::config::Config;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_valid_basic_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("valid.yaml");
    
    let config_content = r#"
repository: "myorg/templates"
tag: "main"
pulls:
  - source: "docker/nodejs"
    target: "./docker"
    type: "directory"
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.repository.unwrap(), "myorg/templates");
    assert_eq!(config.tag.unwrap(), "main");
    assert_eq!(config.pulls.len(), 1);
    assert_eq!(config.pulls[0].source, "docker/nodejs");
    assert_eq!(config.pulls[0].target, "./docker");
    assert_eq!(config.pulls[0].pull_type, "directory");
}

#[test]
fn test_config_with_replacements() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("replacements.yaml");
    
    // Set up environment variable for test
    std::env::set_var("TEST_ENV_VAR", "test_value");
    
    let config_content = r#"
repository: "myorg/templates"
pulls:
  - source: "app/service"
    target: "./service"
    replacements:
      - source: "{{NAME}}"
        target: "my-service"
      - source: "{{VERSION}}"
        valueFromEnv: "TEST_ENV_VAR"
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.pulls[0].replacements.len(), 2);
    assert_eq!(config.pulls[0].replacements[0].source, "{{NAME}}");
    assert_eq!(config.pulls[0].replacements[0].target.as_ref().unwrap(), "my-service");
    assert_eq!(config.pulls[0].replacements[1].value_from_env.as_ref().unwrap(), "TEST_ENV_VAR");
}

#[test]
fn test_config_validation_invalid_schema() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid_schema.yaml");
    
    let config_content = r#"
repository: "myorg/templates"
pulls:
  - source: "docker/nodejs"
    target: "./docker"
    type: "invalid_type"  # Should be "file" or "directory"
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_err());
    
    let error = config.unwrap_err();
    assert!(error.to_string().contains("validation failed"));
}

#[test]
fn test_config_validation_missing_env_var() {
    // Ensure the environment variable doesn't exist
    std::env::remove_var("NONEXISTENT_VAR");
    
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("missing_env.yaml");
    
    let config_content = r#"
repository: "myorg/templates"
pulls:
  - source: "app/service"
    target: "./service"
    replacements:
      - source: "{{VERSION}}"
        valueFromEnv: "NONEXISTENT_VAR"
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_err());
    
    let error = config.unwrap_err();
    assert!(error.to_string().contains("Environment variable"));
}

#[test]
fn test_minimal_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("minimal.yaml");
    
    let config_content = r#"
pulls:
  - source: "src"
    target: "./dest"
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert!(config.repository.is_none());
    assert!(config.tag.is_none());
    assert_eq!(config.pulls.len(), 1);
}

#[test]
fn test_config_directory_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("unsafe.yaml");
    
    let config_content = r#"
repository: "myorg/templates"
pulls:
  - source: "app"
    target: "../../etc/passwd"  # Path traversal attempt
"#;
    
    fs::write(&config_path, config_content).unwrap();
    
    let config = Config::load_from_file(config_path.to_str().unwrap());
    assert!(config.is_err());
    
    let error = config.unwrap_err();
    assert!(error.to_string().contains("unsafe directory traversal"));
}

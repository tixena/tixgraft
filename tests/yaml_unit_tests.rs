//! Unit tests for YAML configuration loading

use tixgraft::config::yaml::load_config;
use tixgraft::system::MockSystem;

#[test]
fn test_load_valid_config() {
    let config_content = r#"
repository: "myorg/scaffolds"
tag: "main"
pulls:
  - source: "kubernetes/mongodb"
    target: "./k8s/mongodb"
    type: "directory"
"#;

    let system = MockSystem::new().with_file("/test/config.yaml", config_content.as_bytes());

    let result = load_config(&system, "/test/config.yaml");
    assert!(result.is_ok());
}

#[test]
fn test_load_nonexistent_file() {
    let system = MockSystem::new();
    let result = load_config(&system, "/nonexistent/file.yaml");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Configuration file not found")
    );
}

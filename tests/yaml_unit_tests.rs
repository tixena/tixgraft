//! Unit tests for YAML configuration loading

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use tixgraft::config::yaml::load_config;
    use tixgraft::system::mock::MockSystem;

    #[test]
    fn load_valid_config() {
        let config_content = r#"
repository: "my_organization/scaffolds"
tag: "main"
pulls:
  - source: "kubernetes/mongodb"
    target: "./k8s/mongodb"
    type: "directory"
"#;

        let system = MockSystem::new()
            .with_file("/test/config.yaml", config_content.as_bytes())
            .unwrap();

        let result = load_config(&system, "/test/config.yaml");
        result.unwrap();
    }

    #[test]
    fn load_nonexistent_file() {
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
}

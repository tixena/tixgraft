//! Tests for `load_config` with a valid file, a missing one, and YAML that does not parse.

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::mock::MemorySystem;
use tixgraft::config::yaml::load_config;

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

    let system = MemorySystem::new()
        .with_file("/test/config.yaml", config_content.as_bytes())
        .unwrap();

    let result = load_config(&system, "/test/config.yaml");
    result.unwrap();
}

#[test]
fn load_nonexistent_file() {
    let system = MemorySystem::new();
    let result = load_config(&system, "/nonexistent/file.yaml");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Configuration file not found")
    );
}

#[test]
fn parse_error_names_the_line_without_printing_it() {
    let config_content = "repository: https://user:hunter2@example.com/r.git\npulls: nope\n";

    let system = MemorySystem::new()
        .with_file("/test/config.yaml", config_content.as_bytes())
        .unwrap();

    let err = load_config(&system, "/test/config.yaml").unwrap_err();
    let message = format!("{err:#}");

    assert!(message.contains("line 2"), "{message}");
    assert!(!message.contains("hunter2"), "{message}");
}

#[test]
fn merge_keys_are_expanded() {
    let config_content = "\
x-base: &base {source: one, target: ./one}
repository: my_organization/scaffolds
pulls:
  - <<: *base
    tag: v1
";

    let system = MemorySystem::new()
        .with_file("/test/config.yaml", config_content.as_bytes())
        .unwrap();

    let config = load_config(&system, "/test/config.yaml").unwrap();
    let pull = config.pulls.first().unwrap();

    assert_eq!(pull.target, "./one");
    assert_eq!(pull.tag.as_deref(), Some("v1"));
}

#[test]
fn alias_expansion_past_the_node_limit_is_rejected() {
    let config_content = "\
a0: &a0 [x, x, x, x, x, x, x, x, x]
a1: &a1 [*a0, *a0, *a0, *a0, *a0, *a0, *a0, *a0, *a0]
a2: &a2 [*a1, *a1, *a1, *a1, *a1, *a1, *a1, *a1, *a1]
a3: &a3 [*a2, *a2, *a2, *a2, *a2, *a2, *a2, *a2, *a2]
a4: &a4 [*a3, *a3, *a3, *a3, *a3, *a3, *a3, *a3, *a3]
a5: &a5 [*a4, *a4, *a4, *a4, *a4, *a4, *a4, *a4, *a4]
repository: my_organization/scaffolds
pulls:
  - {source: one, target: ./one}
";

    let system = MemorySystem::new()
        .with_file("/test/config.yaml", config_content.as_bytes())
        .unwrap();

    let err = load_config(&system, "/test/config.yaml").unwrap_err();
    let message = format!("{err:#}");

    assert!(message.contains("too large or too complex"), "{message}");
}

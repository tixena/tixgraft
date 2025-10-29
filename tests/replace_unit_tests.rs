//! Unit tests for text replacement operations

use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use tixgraft::cli::ReplacementConfig;
use tixgraft::config::graft_yaml::GraftReplacement;
use tixgraft::operations::replace::{
    apply_graft_replacements, apply_single_replacement, get_graft_replacement_value,
    get_replacement_value,
};
use tixgraft::system::{MockSystem, System};

#[test]
fn test_apply_simple_replacement() {
    let system =
        MockSystem::new().with_file("/test.txt", b"Hello {{NAME}}, welcome to {{PLACE}}!\n");

    // Create replacement config
    let replacement = ReplacementConfig {
        source: "{{NAME}}".to_string(),
        target: Some("Alice".to_string()),
        value_from_env: None,
    };

    // Apply replacement
    let result = apply_single_replacement(
        &system,
        Path::new("/test.txt"),
        &replacement.source,
        &replacement.target.as_ref().unwrap(),
    );

    assert!(result.is_ok());

    // Verify replacement was applied
    let content = system.read_to_string(Path::new("/test.txt")).unwrap();
    assert!(content.contains("Hello Alice"));
    assert!(content.contains("{{PLACE}}"));
}

#[test]
fn test_replacement_with_env_var() {
    let system = MockSystem::new().with_env("TEST_ENV", "TestValue");

    let replacement = ReplacementConfig {
        source: "{{TEST}}".to_string(),
        target: None,
        value_from_env: Some("TEST_ENV".to_string()),
    };

    let value = get_replacement_value(&system, &replacement);
    assert!(value.is_ok());
    assert_eq!(value.unwrap(), "TestValue");
}

#[test]
fn test_graft_replacement_with_context() {
    let system = MockSystem::new();
    let mut context = HashMap::new();
    context.insert("projectName".to_string(), json!("my-app"));
    context.insert("maxGb".to_string(), json!(16));

    // Test string context value
    let replacement = GraftReplacement {
        source: "{{PROJECT}}".to_string(),
        target: None,
        value_from_env: None,
        value_from_context: Some("projectName".to_string()),
    };

    let value = get_graft_replacement_value(&system, &replacement, &context);
    assert!(value.is_ok());
    assert_eq!(value.unwrap(), "my-app");

    // Test number context value
    let replacement = GraftReplacement {
        source: "{{MAX_GB}}".to_string(),
        target: None,
        value_from_env: None,
        value_from_context: Some("maxGb".to_string()),
    };

    let value = get_graft_replacement_value(&system, &replacement, &context);
    assert!(value.is_ok());
    assert_eq!(value.unwrap(), "16");
}

#[test]
fn test_graft_replacement_missing_context() {
    let system = MockSystem::new();
    let context = HashMap::new();

    let replacement = GraftReplacement {
        source: "{{VAR}}".to_string(),
        target: None,
        value_from_env: None,
        value_from_context: Some("missing".to_string()),
    };

    let value = get_graft_replacement_value(&system, &replacement, &context);
    assert!(value.is_err());
    assert!(
        value
            .unwrap_err()
            .to_string()
            .contains("Context property 'missing' not found")
    );
}

#[test]
fn test_apply_graft_replacements() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/test.txt", b"Hello {{NAME}}, value is {{VALUE}}!");

    let mut context = HashMap::new();
    context.insert("name".to_string(), json!("Alice"));
    context.insert("value".to_string(), json!(42));

    let replacements = vec![
        GraftReplacement {
            source: "{{NAME}}".to_string(),
            target: None,
            value_from_env: None,
            value_from_context: Some("name".to_string()),
        },
        GraftReplacement {
            source: "{{VALUE}}".to_string(),
            target: None,
            value_from_env: None,
            value_from_context: Some("value".to_string()),
        },
    ];

    let result = apply_graft_replacements(&system, "/test", &replacements, &context);
    assert!(result.is_ok());

    let content = system.read_to_string(Path::new("/test/test.txt")).unwrap();
    assert_eq!(content, "Hello Alice, value is 42!");
}

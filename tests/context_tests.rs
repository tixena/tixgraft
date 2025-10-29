//! Unit tests for context feature
//!
//! Tests the context workflow including:
//! - .graft.yaml discovery and parsing
//! - Context validation (required/optional properties)
//! - Text replacements from context

use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use tixgraft::config::context::ValidatedContext;
use tixgraft::config::graft_yaml::GraftConfig;
use tixgraft::operations::{apply_graft_replacements, copy_files};
use tixgraft::system::{MockSystem, System};

#[test]
fn test_context_basic_flow() {
    let graft_content = r#"
context:
  - name: projectName
    description: The project name
    dataType: string
  - name: version
    description: Version number
    dataType: string
    defaultValue: "1.0.0"

replacements:
  - source: "{{PROJECT_NAME}}"
    valueFromContext: projectName
  - source: "{{VERSION}}"
    valueFromContext: version
"#;

    let template_content = "Project: {{PROJECT_NAME}}\nVersion: {{VERSION}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/config.txt", template_content.as_bytes())
        .with_dir("/target");

    // Load graft config from string
    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    // Create context values
    let mut context = HashMap::new();
    context.insert("projectName".to_string(), json!("MyApp"));

    // Validate context
    let validated = ValidatedContext::new(graft.context.clone(), context).unwrap();

    // Copy file
    copy_files(
        &system,
        Path::new("/source/config.txt"),
        "/target/config.txt",
        "file",
        false,
    )
    .unwrap();

    // Apply replacements
    apply_graft_replacements(
        &system,
        "/target/config.txt",
        &graft.replacements,
        &validated.values,
    )
    .unwrap();

    // Verify result
    let result = system
        .read_to_string(Path::new("/target/config.txt"))
        .unwrap();
    assert_eq!(result, "Project: MyApp\nVersion: 1.0.0");
}

#[test]
fn test_context_required_property_missing() {
    let graft_content = r#"
context:
  - name: required
    description: A required property
    dataType: string

replacements:
  - source: "{{VAR}}"
    valueFromContext: required
"#;

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    // Empty context - should fail validation
    let context = HashMap::new();
    let result = ValidatedContext::new(graft.context.clone(), context);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Missing required context")
    );
}

#[test]
fn test_context_type_coercion() {
    let graft_content = r#"
context:
  - name: port
    description: Port number
    dataType: number
  - name: enabled
    description: Feature enabled
    dataType: boolean

replacements:
  - source: "{{PORT}}"
    valueFromContext: port
  - source: "{{ENABLED}}"
    valueFromContext: enabled
"#;

    let template_content = "Port: {{PORT}}\nEnabled: {{ENABLED}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/file.txt", template_content.as_bytes())
        .with_dir("/target");

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    // String values should coerce to number/boolean
    let mut context = HashMap::new();
    context.insert("port".to_string(), json!("8080"));
    context.insert("enabled".to_string(), json!("true"));

    let validated = ValidatedContext::new(graft.context.clone(), context).unwrap();

    // Copy and replace
    copy_files(
        &system,
        Path::new("/source/file.txt"),
        "/target/file.txt",
        "file",
        false,
    )
    .unwrap();
    apply_graft_replacements(&system, "/target", &graft.replacements, &validated.values).unwrap();

    let result = system
        .read_to_string(Path::new("/target/file.txt"))
        .unwrap();
    assert_eq!(result, "Port: 8080\nEnabled: true");
}

#[test]
fn test_context_array_json() {
    let graft_content = r#"
context:
  - name: services
    description: List of services
    dataType: array

replacements:
  - source: "{{SERVICES}}"
    valueFromContext: services
"#;

    let template_content = "Services: {{SERVICES}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/file.txt", template_content.as_bytes())
        .with_dir("/target");

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    let services = json!([
        {"name": "api", "port": 8080},
        {"name": "web", "port": 3000}
    ]);

    let mut context = HashMap::new();
    context.insert("services".to_string(), services);

    let validated = ValidatedContext::new(graft.context.clone(), context).unwrap();

    copy_files(
        &system,
        Path::new("/source/file.txt"),
        "/target/file.txt",
        "file",
        false,
    )
    .unwrap();
    apply_graft_replacements(&system, "/target", &graft.replacements, &validated.values).unwrap();

    let result = system
        .read_to_string(Path::new("/target/file.txt"))
        .unwrap();
    assert!(result.contains("api"));
    assert!(result.contains("8080"));
    assert!(result.contains("web"));
    assert!(result.contains("3000"));
}

#[test]
fn test_context_default_values() {
    let graft_content = r#"
context:
  - name: name
    description: Application name
    dataType: string
  - name: version
    description: Version number
    dataType: string
    defaultValue: "1.0.0"

replacements:
  - source: "{{NAME}}"
    valueFromContext: name
  - source: "{{VERSION}}"
    valueFromContext: version
"#;

    let template_content = "Name: {{NAME}}\nVersion: {{VERSION}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/file.txt", template_content.as_bytes())
        .with_dir("/target");

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    // Only provide required property
    let mut context = HashMap::new();
    context.insert("name".to_string(), json!("MyApp"));

    let validated = ValidatedContext::new(graft.context.clone(), context).unwrap();

    copy_files(
        &system,
        Path::new("/source/file.txt"),
        "/target/file.txt",
        "file",
        false,
    )
    .unwrap();
    apply_graft_replacements(&system, "/target", &graft.replacements, &validated.values).unwrap();

    let result = system
        .read_to_string(Path::new("/target/file.txt"))
        .unwrap();
    assert_eq!(result, "Name: MyApp\nVersion: 1.0.0");
}

#[test]
fn test_invalid_graft_yaml_syntax() {
    let graft_content = r#"
context:
  - name: test
    description: "Missing closing quote
"#;

    let result = GraftConfig::load_from_string(graft_content);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();

    // Error should include line number information
    assert!(
        error_msg.contains("line") && error_msg.contains("column"),
        "Error message should include line and column numbers, got: {}",
        error_msg
    );
    assert!(
        error_msg.contains("Failed to parse .graft.yaml"),
        "Error message should indicate YAML parsing failure, got: {}",
        error_msg
    );
}

#[test]
fn test_yaml_syntax_error_reports_exact_location() {
    // Test various YAML syntax errors to ensure line numbers are reported

    // Case 1: Missing closing bracket
    let yaml1 = r#"
context:
  - name: test
    description: "test
"#;
    let result1 = GraftConfig::load_from_string(yaml1);
    assert!(result1.is_err());
    let error1 = result1.unwrap_err().to_string();
    assert!(error1.contains("line") && error1.contains("column"));

    // Case 2: Invalid indentation
    let yaml2 = r#"
context:
- name: test
  description: correct
 bad_indent: wrong
"#;
    let result2 = GraftConfig::load_from_string(yaml2);
    assert!(result2.is_err());
    let error2 = result2.unwrap_err().to_string();
    assert!(error2.contains("line") && error2.contains("column"));
}

#[test]
fn test_yaml_type_error_with_location() {
    // YAML parses but validation fails - still should have helpful error
    let graft_content = r#"
context:
  - name: test
    description: "Test"
    dataType: invalid_type
"#;

    let result = GraftConfig::load_from_string(graft_content);
    // This should fail validation, not parsing
    assert!(result.is_err());
}

#[test]
fn test_replacement_value_not_in_context() {
    let graft_content = r#"
replacements:
  - source: "{{VAR}}"
    valueFromContext: nonexistent
"#;

    let template_content = "{{VAR}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/file.txt", template_content.as_bytes())
        .with_dir("/target");

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    let _context: HashMap<String, serde_json::Value> = HashMap::new();
    let validated_values: HashMap<String, serde_json::Value> = HashMap::new();

    copy_files(
        &system,
        Path::new("/source/file.txt"),
        "/target/file.txt",
        "file",
        false,
    )
    .unwrap();

    let result =
        apply_graft_replacements(&system, "/target", &graft.replacements, &validated_values);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Context property") || err_msg.contains("nonexistent"),
        "Expected error about missing context property, got: {}",
        err_msg
    );
}

#[test]
fn test_context_mixed_sources() {
    let graft_content = r#"
context:
  - name: ctx
    description: From context
    dataType: string

replacements:
  - source: "{{CTX}}"
    valueFromContext: ctx
  - source: "{{STATIC}}"
    target: "StaticValue"
"#;

    let template_content = "Context: {{CTX}}\nStatic: {{STATIC}}";

    let system = MockSystem::new()
        .with_dir("/source")
        .with_file("/source/file.txt", template_content.as_bytes())
        .with_dir("/target");

    let graft = GraftConfig::load_from_string(graft_content).unwrap();

    let mut context = HashMap::new();
    context.insert("ctx".to_string(), json!("FromContext"));

    let validated = ValidatedContext::new(graft.context.clone(), context).unwrap();

    copy_files(
        &system,
        Path::new("/source/file.txt"),
        "/target/file.txt",
        "file",
        false,
    )
    .unwrap();
    apply_graft_replacements(&system, "/target", &graft.replacements, &validated.values).unwrap();

    let result = system
        .read_to_string(Path::new("/target/file.txt"))
        .unwrap();
    assert_eq!(result, "Context: FromContext\nStatic: StaticValue");
}

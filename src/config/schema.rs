//! JSON Schema validation for tixgraft configuration

use anyhow::{Result, anyhow};
use jsonschema::{Draft, Validator};
use serde_json::Value;

/// Get the embedded JSON schema for tixgraft configuration
pub fn get_schema() -> Result<Validator> {
    let schema_str = include_str!("../../docs/schema.json");
    let schema: Value = serde_json::from_str(schema_str)
        .map_err(|e| anyhow!("Failed to parse embedded JSON schema: {e}"))?;

    return Validator::options()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .map_err(|e| anyhow!("Failed to compile JSON schema: {e}"));
}

/// Validate a configuration value against the schema
pub fn validate_against_schema(config: &Value) -> Result<()> {
    let schema = get_schema()?;

    if !schema.is_valid(config) {
        let errors: Vec<String> = schema
            .iter_errors(config)
            .map(|e| {
                format!(
                    "  - Path '{}': {} (schema: {})",
                    e.instance_path, e, e.schema_path
                )
            })
            .collect();

        return Err(anyhow!(
            "Configuration validation failed:\n{}",
            errors.join("\n")
        ));
    }

    Ok(())
}

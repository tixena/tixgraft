//! JSON Schema validation for tixgraft configuration.

use anyhow::{Result, anyhow};
use jsonschema::{Draft, Validator};
use serde_json::Value;

/// Get the embedded JSON schema for tixgraft configuration.
///
/// # Errors
///
/// Returns an error if:
/// - The JSON schema cannot be parsed
#[inline]
pub fn get_schema() -> Result<Validator> {
    let schema_str = include_str!("../../docs/schema.json");
    let schema: Value = serde_json::from_str(schema_str)
        .map_err(|err| anyhow!("Failed to parse embedded JSON schema: {err}"))?;

    return Validator::options()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .map_err(|err| anyhow!("Failed to compile JSON schema: {err}"));
}

/// Validate a configuration value against the schema.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration is invalid
#[inline]
pub fn validate_against_schema(config: &Value) -> Result<()> {
    let schema = get_schema()?;

    if !schema.is_valid(config) {
        let errors: Vec<String> = schema
            .iter_errors(config)
            .map(|err| {
                format!(
                    "  - Path '{}': {} (schema: {})",
                    err.instance_path(), err, err.schema_path()
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

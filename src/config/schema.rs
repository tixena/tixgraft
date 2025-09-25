//! JSON Schema validation for tixgraft configuration

use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use anyhow::{Result, anyhow};

/// Get the embedded JSON schema for tixgraft configuration
pub fn get_schema() -> Result<JSONSchema> {
    let schema_str = include_str!("../../docs/schema.json");
    let schema: Value = serde_json::from_str(schema_str)
        .map_err(|e| anyhow!("Failed to parse embedded JSON schema: {}", e))?;
    
    JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&schema)
        .map_err(|e| anyhow!("Failed to compile JSON schema: {}", e))
}

/// Validate a configuration value against the schema
pub fn validate_against_schema(config: &Value) -> Result<()> {
    let schema = get_schema()?;
    
    if let Err(errors) = schema.validate(config) {
        let error_messages: Vec<String> = errors
            .map(|e| format!("  - Path '{}': {} (schema: {})", e.instance_path, e, e.schema_path))
            .collect();
        
        return Err(anyhow!(
            "Configuration validation failed:\n{}",
            error_messages.join("\n")
        ));
    }
    
    Ok(())
}

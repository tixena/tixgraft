//! Parser for .graft.yaml files.
//!
//! Handles parsing of .graft.yaml files which define context requirements,
//! replacements, and post-commands for grafts.

use crate::config::context::{ContextDataType, ContextPropertyDefinition};
use crate::error::GraftError;
use anyhow::{Context as _, Result};
use os_shim::System;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Complete .graft.yaml configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftConfig {
    /// Context property definitions.
    #[serde(default)]
    pub context: Vec<ContextPropertyDefinition>,

    /// Post-commands to execute after replacements.
    #[serde(default)]
    pub post_commands: Vec<PostCommand>,

    /// Text replacements.
    #[serde(default)]
    pub replacements: Vec<GraftReplacement>,
}

/// Replacement configuration in .graft.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftReplacement {
    /// Source pattern to search for.
    pub source: String,

    /// Static replacement value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Context property to get value from.
    #[serde(rename = "valueFromContext", skip_serializing_if = "Option::is_none")]
    pub value_from_context: Option<String>,

    /// Environment variable to get value from.
    #[serde(rename = "valueFromEnv", skip_serializing_if = "Option::is_none")]
    pub value_from_env: Option<String>,
}

impl GraftReplacement {
    #[must_use]
    #[inline]
    pub const fn new(
        source: String,
        target: Option<String>,
        value_from_env: Option<String>,
        value_from_context: Option<String>,
    ) -> Self {
        Self {
            source,
            target,
            value_from_context,
            value_from_env,
        }
    }
}

/// Post-command configuration (enum for different types).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum PostCommand {
    /// Conditional choice based on test command.
    Choice { options: Vec<ChoiceOption> },

    /// Simple command execution.
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
}

impl PostCommand {
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self::Command { command, args, cwd }
    }
}

/// Choice option for conditional execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ChoiceOption {
    /// Expected output pattern to match.
    pub expected_output: String,

    /// Command to execute if match succeeds.
    pub on_match: Box<PostCommand>,

    /// Test command to run.
    pub test: TestCommand,
}

impl ChoiceOption {
    #[must_use]
    #[inline]
    pub const fn new(
        test: TestCommand,
        expected_output: String,
        on_match: Box<PostCommand>,
    ) -> Self {
        Self {
            expected_output,
            on_match,
            test,
        }
    }
}

/// Test command for conditional execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TestCommand {
    #[serde(default)]
    pub args: Vec<String>,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl TestCommand {
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self { args, command, cwd }
    }
}

impl GraftConfig {
    /// Load .graft.yaml from file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `.graft.yaml` file cannot be found
    /// - The `.graft.yaml` file cannot be read
    /// - The `.graft.yaml` configuration is invalid
    #[inline]
    pub fn load_from_file(system: &dyn System, path: &Path) -> Result<Self> {
        if !system.exists(path)? {
            return Err(GraftError::configuration(format!(
                ".graft.yaml not found: {}",
                path.display()
            ))
            .into());
        }

        let content = system
            .read_to_string(path)
            .with_context(|| format!("Failed to read .graft.yaml file: {}", path.display()))?;

        Self::load_from_string(&content)
    }

    /// Load .graft.yaml from string content.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `.graft.yaml` configuration is invalid
    #[inline]
    pub fn load_from_string(content: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(content).map_err(|err| {
            // Extract line and column information from serde_yaml error
            err.location().map_or_else(
                || anyhow::anyhow!("Failed to parse .graft.yaml: {err}"),
                |location| {
                    anyhow::anyhow!(
                        "Failed to parse .graft.yaml at line {}, column {}: {}",
                        location.line(),
                        location.column(),
                        err
                    )
                },
            )
        })?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate the .graft.yaml configuration.
    fn validate(&self) -> Result<()> {
        // Validate context definitions
        for def in &self.context {
            if def.name.is_empty() {
                return Err(GraftError::configuration(
                    "Context property name cannot be empty".to_owned(),
                )
                .into());
            }
            if def.description.is_empty() {
                return Err(GraftError::configuration(format!(
                    "Context property '{}' must have a description",
                    def.name
                ))
                .into());
            }

            // Validate default value type matches data_type
            if let Some(default_value) = def.default_value.as_ref() {
                validate_value_type(&def.name, default_value, &def.data_type)?;
            }
        }

        // Validate replacements
        for replacement in &self.replacements {
            let source_count = u8::from(replacement.target.is_some())
                .checked_add(u8::from(replacement.value_from_env.is_some()))
                .and_then(|sum| sum.checked_add(u8::from(replacement.value_from_context.is_some())))
                .unwrap_or(u8::MAX);

            if source_count != 1_u8 {
                return Err(GraftError::configuration(
                    format!(
                        "Replacement for '{}' must specify exactly one of: target, valueFromEnv, or valueFromContext",
                        replacement.source
                    )
                )
                .into());
            }
        }

        Ok(())
    }
}

/// Default implementation for `PostCommand` when not specified.
impl Default for PostCommand {
    #[inline]
    fn default() -> Self {
        Self::Command {
            command: String::new(),
            args: Vec::new(),
            cwd: None,
        }
    }
}

// Custom deserializer for PostCommand to handle missing 'type' field
// (defaults to 'command' type when 'type' is not specified)
#[expect(
    clippy::missing_trait_methods,
    reason = "custom Deserialize impl only needs deserialize method"
)]
impl<'de> Deserialize<'de> for PostCommand {
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `PostCommand` cannot be deserialized
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        let value = Value::deserialize(deserializer)?;
        let obj = value
            .as_object()
            .ok_or_else(|| D::Error::custom("Expected object"))?;

        // Check if 'type' field exists
        if let Some(type_value) = obj.get("type") {
            let type_str = type_value
                .as_str()
                .ok_or_else(|| D::Error::custom("'type' field must be a string"))?;

            match type_str {
                "command" => {
                    // Parse as command type
                    let command = obj
                        .get("command")
                        .and_then(|val| val.as_str())
                        .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                        .to_owned();

                    let args = obj
                        .get("args")
                        .and_then(|val| val.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|val| val.as_str().map(ToOwned::to_owned))
                                .collect()
                        })
                        .unwrap_or_default();

                    let cwd = obj
                        .get("cwd")
                        .and_then(|val| val.as_str())
                        .map(ToOwned::to_owned);

                    Ok(Self::Command { command, args, cwd })
                }
                "choice" => {
                    // Parse as choice type
                    let options_value = obj.get("options").ok_or_else(|| {
                        D::Error::custom("Missing 'options' field for choice type")
                    })?;

                    let options: Vec<ChoiceOption> = serde_json::from_value(options_value.clone())
                        .map_err(|err| {
                            D::Error::custom(format!("Failed to parse options: {err}"))
                        })?;

                    Ok(Self::Choice { options })
                }
                _ => Err(D::Error::custom(format!(
                    "Unknown PostCommand type: {type_str}"
                ))),
            }
        } else {
            // No 'type' field, default to 'command' type
            let command = obj
                .get("command")
                .and_then(|val| val.as_str())
                .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                .to_owned();

            let args = obj
                .get("args")
                .and_then(|val| val.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|val| val.as_str().map(ToOwned::to_owned))
                        .collect()
                })
                .unwrap_or_default();

            let cwd = obj
                .get("cwd")
                .and_then(|val| val.as_str())
                .map(ToOwned::to_owned);

            Ok(Self::Command { command, args, cwd })
        }
    }
}

/// Validate that a value matches the expected data type.
fn validate_value_type(name: &str, value: &Value, expected_type: &ContextDataType) -> Result<()> {
    let matches = match *expected_type {
        ContextDataType::String => value.is_string(),
        ContextDataType::Number => value.is_number(),
        ContextDataType::Boolean => value.is_boolean(),
        ContextDataType::Array => value.is_array(),
    };

    if !matches {
        return Err(GraftError::configuration(format!(
            "Default value for property '{name}' has wrong type (expected {expected_type:?}, got {value:?})"
        ))
        .into());
    }

    Ok(())
}

/// Tests for private functions only. Public API tests are in `tests/graft_yaml_unit_tests.rs`.
#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "unwrap is acceptable in test code for brevity"
)]
mod tests {
    use super::*;

    #[test]
    fn context_property_validation() {
        let yaml = r#"
context:
  - name: ""
    description: "Empty name"
    dataType: string
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        config.validate().unwrap_err();
    }

    #[test]
    fn default_value_type_validation() {
        let yaml = r#"
context:
  - name: port
    description: Port number
    dataType: number
    defaultValue: "not-a-number"
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        config.validate().unwrap_err();
    }

    #[test]
    fn context_empty_description() {
        let yaml = "context:\n  - name: myProp\n    description: \"\"\n    dataType: string\n";
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        config.validate().unwrap_err();
    }

    #[test]
    fn validate_value_type_tst() {
        use serde_json::json;
        validate_value_type("x", &json!("hello"), &ContextDataType::String).unwrap();
        validate_value_type("x", &json!(42_i64), &ContextDataType::String).unwrap_err();
        validate_value_type("x", &json!(42_i64), &ContextDataType::Number).unwrap();
        validate_value_type("x", &json!(true), &ContextDataType::Boolean).unwrap();
        validate_value_type("x", &json!("hi"), &ContextDataType::Boolean).unwrap_err();
        validate_value_type("x", &json!([1_i32, 2_i32]), &ContextDataType::Array).unwrap();
        validate_value_type("x", &json!("str"), &ContextDataType::Array).unwrap_err();
    }
}

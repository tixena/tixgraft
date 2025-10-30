//! Parser for .graft.yaml files
//!
//! Handles parsing of .graft.yaml files which define context requirements,
//! replacements, and post-commands for grafts.

use crate::config::context::{ContextDataType, ContextPropertyDefinition};
use crate::error::GraftError;
use crate::system::System;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Complete .graft.yaml configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftConfig {
    /// Context property definitions
    #[serde(default)]
    pub context: Vec<ContextPropertyDefinition>,

    /// Text replacements
    #[serde(default)]
    pub replacements: Vec<GraftReplacement>,

    /// Post-commands to execute after replacements
    #[serde(default)]
    pub post_commands: Vec<PostCommand>,
}

/// Replacement configuration in .graft.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftReplacement {
    /// Source pattern to search for
    pub source: String,

    /// Static replacement value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Environment variable to get value from
    #[serde(rename = "valueFromEnv", skip_serializing_if = "Option::is_none")]
    pub value_from_env: Option<String>,

    /// Context property to get value from
    #[serde(rename = "valueFromContext", skip_serializing_if = "Option::is_none")]
    pub value_from_context: Option<String>,
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
            value_from_env,
            value_from_context,
        }
    }
}

/// Post-command configuration (enum for different types)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum PostCommand {
    /// Simple command execution
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },

    /// Conditional choice based on test command
    Choice { options: Vec<ChoiceOption> },
}

impl PostCommand {
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self::Command { command, args, cwd }
    }
}

/// Choice option for conditional execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ChoiceOption {
    /// Test command to run
    pub test: TestCommand,

    /// Expected output pattern to match
    pub expected_output: String,

    /// Command to execute if match succeeds
    pub on_match: Box<PostCommand>,
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
            test,
            expected_output,
            on_match,
        }
    }
}

/// Test command for conditional execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TestCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl TestCommand {
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self { command, args, cwd }
    }
}

impl GraftConfig {
    /// Load .graft.yaml from string content
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `.graft.yaml` configuration is invalid
    #[inline]
    pub fn load_from_string(content: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(content).map_err(|e| {
            // Extract line and column information from serde_yaml error
            if let Some(location) = e.location() {
                anyhow::anyhow!(
                    "Failed to parse .graft.yaml at line {}, column {}: {}",
                    location.line(),
                    location.column(),
                    e
                )
            } else {
                anyhow::anyhow!("Failed to parse .graft.yaml: {e}")
            }
        })?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// Load .graft.yaml from file
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

    /// Validate the .graft.yaml configuration
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
            let mut count = 0;
            if replacement.target.is_some() {
                count += 1;
            }
            if replacement.value_from_env.is_some() {
                count += 1;
            }
            if replacement.value_from_context.is_some() {
                count += 1;
            }

            if count != 1 {
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

/// Validate that a value matches the expected data type
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

/// Default implementation for `PostCommand` when not specified
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
#[expect(clippy::missing_trait_methods)]
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
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                        .to_owned();

                    let args = obj
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                                .collect()
                        })
                        .unwrap_or_default();

                    let cwd = obj
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned);

                    Ok(Self::Command { command, args, cwd })
                }
                "choice" => {
                    // Parse as choice type
                    let options_value = obj.get("options").ok_or_else(|| {
                        D::Error::custom("Missing 'options' field for choice type")
                    })?;

                    let options: Vec<ChoiceOption> = serde_json::from_value(options_value.clone())
                        .map_err(|e| D::Error::custom(format!("Failed to parse options: {e}")))?;

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
                .and_then(|v| v.as_str())
                .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                .to_owned();

            let args = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                        .collect()
                })
                .unwrap_or_default();

            let cwd = obj
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned);

            Ok(Self::Command { command, args, cwd })
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::system::{mock::MockSystem, real::RealSystem};

    use super::*;
    use std::io::Write as _;
    use tempfile::NamedTempFile;

    #[test]
    fn load_valid_graft_config() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
context:
  - name: projectName
    description: The project name
    dataType: string

  - name: maxGbPerPod
    description: Max GB per pod
    dataType: number
    defaultValue: 10

replacements:
  - source: "{{PROJECT_NAME}}"
    valueFromContext: projectName

  - source: "{{MAX_GB}}"
    valueFromContext: maxGbPerPod

postCommands:
  - command: echo
    args: ["Hello"]
"#
        )
        .unwrap();

        let system = RealSystem::new();
        let result = GraftConfig::load_from_file(&system, temp_file.path());
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.context.len(), 2);
        assert_eq!(config.replacements.len(), 2);
        assert_eq!(config.post_commands.len(), 1);
    }

    #[test]
    fn validate_replacement_exclusivity() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
replacements:
  - source: "{{VAR}}"
    target: "value"
    valueFromEnv: "ENV_VAR"
"#
        )
        .unwrap();

        let system = RealSystem::new();
        let result = GraftConfig::load_from_file(&system, temp_file.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must specify exactly one of")
        );
    }

    #[test]
    #[expect(clippy::panic)]
    fn post_command_default_type() {
        let yaml = r#"
postCommands:
  - command: npm
    args: ["install"]
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.post_commands.len(), 1);
        match config.post_commands[0] {
            PostCommand::Command { ref command, .. } => {
                assert_eq!(command, "npm");
            }
            PostCommand::Choice { .. } => panic!("Expected Command type"),
        }
    }

    #[test]
    #[expect(clippy::panic)]
    fn post_command_choice_type() {
        let yaml = r#"
postCommands:
  - type: choice
    options:
      - test:
          command: node
          args: ["--version"]
        expectedOutput: "v"
        onMatch:
          command: npm
          args: ["install"]
"#;
        let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.post_commands.len(), 1);
        match config.post_commands[0] {
            PostCommand::Choice { ref options } => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].test.command, "node");
                assert_eq!(options[0].expected_output, "v");
            }
            PostCommand::Command { .. } => panic!("Expected Choice type"),
        }
    }

    #[test]
    fn context_property_validation() {
        let yaml = r#"
context:
  - name: ""
    description: "Empty name"
    dataType: string
"#;
        let result: Result<GraftConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.validate().is_err());
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
        let result: Result<GraftConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn load_from_file_not_found() {
        let system = MockSystem::new();
        let result = GraftConfig::load_from_file(&system, Path::new("/nonexistent/.graft.yaml"));

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains(".graft.yaml not found"),
            "Error should indicate file not found, got: {error_msg}"
        );
    }

    #[test]
    fn load_from_file_with_mock() {
        let yaml_content = r#"
context:
  - name: test
    description: Test variable
    dataType: string
replacements:
  - source: "{{TEST}}"
    target: "value"
"#;

        let system = MockSystem::new()
            .with_file("/test/.graft.yaml", yaml_content.as_bytes())
            .unwrap();

        let result = GraftConfig::load_from_file(&system, Path::new("/test/.graft.yaml"));
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.context.len(), 1);
        assert_eq!(config.context[0].name, "test");
        assert_eq!(config.replacements.len(), 1);
    }
}

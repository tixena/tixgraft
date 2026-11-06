//! Parser for `.graft.yaml` files, which define a graft's context requirements, replacements and
//! post-commands.
//!
//! Parsing also checks the rules serde cannot: non-empty names and descriptions, defaults of the
//! declared type and one value source per replacement.

use crate::config::context::{ContextDataType, ContextPropertyDefinition};
use crate::config::yaml::parse_yaml;
use crate::error::GraftError;
use anyhow::{Context as _, Result};
use os_shim::System;
use pollster::FutureExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// The contents of a `.graft.yaml`: the context a graft requires, its replacements and its
/// post-commands.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftConfig {
    /// Context properties the graft declares.
    #[serde(default)]
    pub context: Vec<ContextPropertyDefinition>,

    /// Post-commands to execute after replacements.
    #[serde(default)]
    pub post_commands: Vec<PostCommand>,

    /// Text replacements applied to the grafted files.
    #[serde(default)]
    pub replacements: Vec<GraftReplacement>,
}

/// One replacement in `.graft.yaml`; exactly one of its three value sources must be set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct GraftReplacement {
    /// Literal text to search for.
    pub source: String,

    /// Static replacement value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Context property whose value replaces the source.
    #[serde(rename = "valueFromContext", skip_serializing_if = "Option::is_none")]
    pub value_from_context: Option<String>,

    /// Environment variable whose value replaces the source.
    #[serde(rename = "valueFromEnv", skip_serializing_if = "Option::is_none")]
    pub value_from_env: Option<String>,
}

impl GraftReplacement {
    /// Builds a replacement; validation later requires exactly one of the three value sources.
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

/// One step of a graft's `postCommands`; a mapping with no `type` reads as a command.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[non_exhaustive]
pub enum PostCommand {
    /// Runs the step of the first of `options` whose test output matches; no match is a no-op.
    Choice { options: Vec<ChoiceOption> },

    /// Runs one program directly, with no shell.
    Command {
        /// Program to run.
        command: String,
        /// Arguments passed to the program.
        #[serde(default)]
        args: Vec<String>,
        /// Working directory, resolved against the graft's directory, which is also the default.
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
}

impl PostCommand {
    /// Builds the `Command` variant, the one a step with no `type` reads as.
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self::Command { command, args, cwd }
    }
}

/// One option of a `Choice`: a test, the pattern its output must match and the step to run then.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ChoiceOption {
    /// Regular expression matched against the test's standard output.
    pub expected_output: String,

    /// Step to run when the expression matches.
    pub on_match: Box<PostCommand>,

    /// Command whose output is matched.
    pub test: TestCommand,
}

impl ChoiceOption {
    /// Builds an option from its test, the pattern its output must match and the step to run then.
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

/// Command whose standard output decides a choice option; its exit status is not consulted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TestCommand {
    /// Arguments passed to the program.
    #[serde(default)]
    pub args: Vec<String>,
    /// Program to run, directly with no shell.
    pub command: String,
    /// Working directory, resolved against the graft's directory, which is also the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl TestCommand {
    /// Builds a test from its program, arguments and working directory.
    #[must_use]
    #[inline]
    pub const fn new(command: String, args: Vec<String>, cwd: Option<String>) -> Self {
        Self { args, command, cwd }
    }
}

impl GraftConfig {
    /// Reads the file at `path` and parses and validates its content.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is missing or unreadable, or its content is invalid.
    #[inline]
    pub fn load_from_file(system: &dyn System, path: &Path) -> Result<Self> {
        if !system.exists(path).block_on()? {
            return Err(GraftError::configuration(format!(
                ".graft.yaml not found: {}",
                path.display()
            ))
            .into());
        }

        let content = system
            .read_to_string(path)
            .block_on()
            .with_context(|| format!("Failed to read .graft.yaml file: {}", path.display()))?;

        Self::load_from_string(&content)
    }

    /// Parses and validates `content`; a parse error names its line and column when known.
    ///
    /// # Errors
    ///
    /// Returns an error if `content` does not parse or breaks a validation rule.
    #[inline]
    pub fn load_from_string(content: &str) -> Result<Self> {
        let config: Self = parse_yaml(content)
            .map_err(|err| anyhow::anyhow!("Failed to parse .graft.yaml: {err}"))?;

        config.validate()?;

        Ok(config)
    }

    /// Requires a name and description on every property, a default of the declared type, and
    /// exactly one value source on every replacement.
    fn validate(&self) -> Result<()> {
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

            if let Some(default_value) = def.default_value.as_ref() {
                validate_value_type(&def.name, default_value, &def.data_type)?;
            }
        }

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

// Hand-written because serde's internal tagging rejects a mapping with no `type`.
#[expect(
    clippy::missing_trait_methods,
    reason = "custom Deserialize impl only needs deserialize method"
)]
impl<'de> Deserialize<'de> for PostCommand {
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

        if let Some(type_value) = obj.get("type") {
            let type_str = type_value
                .as_str()
                .ok_or_else(|| D::Error::custom("'type' field must be a string"))?;

            match type_str {
                "command" => {
                    let command = obj
                        .get("command")
                        .and_then(|val| val.as_str())
                        .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                        .to_owned();

                    let args = obj
                        .get("args")
                        .and_then(|val| val.as_array())
                        .map_or_default(|arr| {
                            arr.iter()
                                .filter_map(|val| val.as_str().map(ToOwned::to_owned))
                                .collect()
                        });

                    let cwd = obj
                        .get("cwd")
                        .and_then(|val| val.as_str())
                        .map(ToOwned::to_owned);

                    Ok(Self::Command { command, args, cwd })
                }
                "choice" => {
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
            let command = obj
                .get("command")
                .and_then(|val| val.as_str())
                .ok_or_else(|| D::Error::custom("Missing 'command' field"))?
                .to_owned();

            let args = obj
                .get("args")
                .and_then(|val| val.as_array())
                .map_or_default(|arr| {
                    arr.iter()
                        .filter_map(|val| val.as_str().map(ToOwned::to_owned))
                        .collect()
                });

            let cwd = obj
                .get("cwd")
                .and_then(|val| val.as_str())
                .map(ToOwned::to_owned);

            Ok(Self::Command { command, args, cwd })
        }
    }
}

/// Checks a declared default strictly: unlike a supplied value, a default is not coerced.
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

#[cfg(test)]
mod tests;

//! YAML configuration loading and parsing.
//!
//! A loaded file passes the JSON schema and then the validation rules.

use crate::config::schema::validate_against_schema;
use crate::config::{Config, validation::validate_config_with_base_dir};
use anyhow::{Context as _, Result, anyhow};
use os_shim::System;
use pollster::FutureExt as _;
use serde::de::DeserializeOwned;
use serde_saphyr::UserMessageFormatter;
use std::path::Path;

/// Reads and validates the file at `path`, resolving child paths against its directory, so nested
/// configurations validate from any working directory.
///
/// # Errors
///
/// Returns an error if the file is missing, unreadable or unparsable, or fails validation.
#[inline]
pub fn load_config(system: &dyn System, path: &str) -> Result<Config> {
    let path_obj = Path::new(path);

    if !system.exists(path_obj).block_on()? {
        return Err(anyhow!(
            "Configuration file not found: {path}\n\
            Create a tixgraft.yaml file or specify a different path with --config"
        ));
    }

    let content = system
        .read_to_string(path_obj)
        .block_on()
        .with_context(|| format!("Failed to read configuration file: {path}"))?;

    let config: Config = parse_yaml(&content).with_context(|| {
        format!(
            "Failed to parse YAML configuration in file: {path}\n\
            Please check the syntax and structure of your configuration file"
        )
    })?;

    let config_value = serde_json::to_value(&config)
        .context("Failed to convert configuration to JSON for validation")?;

    validate_against_schema(&config_value).context("Configuration validation failed")?;

    let base_dir = Path::new(path).parent();
    validate_config_with_base_dir(system, &config, base_dir)
        .context("Configuration validation failed")?;

    Ok(config)
}

/// Parses YAML into `T`. The error names the line and column but prints no line of the file, which
/// may hold a password.
pub(crate) fn parse_yaml<T>(content: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_saphyr::from_str_with_options(content, serde_saphyr::options! { with_snippet: false })
        .map_err(|err| anyhow!(err.render_with_formatter(&UserMessageFormatter)))
}

//! YAML configuration loading and parsing

use crate::config::Config;
use crate::system::System;
use anyhow::{Context as _, Result, anyhow};
use std::path::Path;

/// Load and parse YAML configuration from file
pub fn load_config(system: &dyn System, path: &str) -> Result<Config> {
    let path_obj = Path::new(path);

    // Check if file exists using System trait
    if !system.exists(path_obj) {
        return Err(anyhow!(
            "Configuration file not found: {path}\n\
            Create a tixgraft.yaml file or specify a different path with --config"
        ));
    }

    // Read file contents using System trait
    let content = system
        .read_to_string(path_obj)
        .with_context(|| format!("Failed to read configuration file: {path}"))?;

    // Parse YAML
    let config: Config = serde_yaml::from_str(&content).with_context(|| {
        return format!(
            "Failed to parse YAML configuration in file: {path}\n\
            Please check the syntax and structure of your configuration file"
        );
    })?;

    // Validate against JSON schema
    let config_value = serde_json::to_value(&config)
        .context("Failed to convert configuration to JSON for validation")?;

    crate::config::schema::validate_against_schema(&config_value)
        .context("Configuration validation failed")?;

    // Validate configuration logic (path safety, env vars, etc.)
    crate::config::validation::validate_config(system, &config)
        .context("Configuration validation failed")?;

    Ok(config)
}

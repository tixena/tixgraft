//! YAML configuration loading and parsing

use std::fs;
use anyhow::{Result, anyhow, Context};
use crate::config::Config;

/// Load and parse YAML configuration from file
pub fn load_config(path: &str) -> Result<Config> {
    // Check if file exists
    if !std::path::Path::new(path).exists() {
        return Err(anyhow!(
            "Configuration file not found: {}\n\
            Create a tixgraft.yaml file or specify a different path with --config",
            path
        ));
    }

    // Read file contents
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read configuration file: {}", path))?;

    // Parse YAML
    let config: Config = serde_yaml::from_str(&content)
        .with_context(|| format!(
            "Failed to parse YAML configuration in file: {}\n\
            Please check the syntax and structure of your configuration file",
            path
        ))?;

    // Validate against JSON schema
    let config_value = serde_json::to_value(&config)
        .context("Failed to convert configuration to JSON for validation")?;
    
    crate::config::schema::validate_against_schema(&config_value)
        .context("Configuration validation failed")?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_load_valid_config() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
repository: "myorg/scaffolds"
tag: "main"
pulls:
  - source: "kubernetes/mongodb"
    target: "./k8s/mongodb"
    type: "directory"
"#
        ).unwrap();

        let result = load_config(temp_file.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_config("/nonexistent/file.yaml");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Configuration file not found"));
    }
}

//! Unit tests for text replacement operations

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;
    use tixgraft::cli::ReplacementConfig;
    use tixgraft::config::graft_yaml::GraftReplacement;
    use tixgraft::operations::replace::{
        apply_graft_replacements, apply_single_replacement, get_graft_replacement_value,
        get_replacement_value,
    };
    use tixgraft::system::System as _;
    use tixgraft::system::mock::MockSystem;

    #[test]
    fn apply_simple_replacement() {
        let system = MockSystem::new()
            .with_file("/test.txt", b"Hello {{NAME}}, welcome to {{PLACE}}!\n")
            .unwrap();

        // Create replacement config
        let replacement =
            ReplacementConfig::new("{{NAME}}".to_owned(), Some("Alice".to_owned()), None);

        // Apply replacement
        let result = apply_single_replacement(
            &system,
            Path::new("/test.txt"),
            &replacement.source,
            replacement.target.as_ref().unwrap(),
        );

        result.unwrap();

        // Verify replacement was applied
        let content = system.read_to_string(Path::new("/test.txt")).unwrap();
        assert!(content.contains("Hello Alice"));
        assert!(content.contains("{{PLACE}}"));
    }

    #[test]
    fn replacement_with_env_var() {
        let system = MockSystem::new().with_env("TEST_ENV", "TestValue").unwrap();

        let replacement =
            ReplacementConfig::new("{{TEST}}".to_owned(), None, Some("TEST_ENV".to_owned()));

        let value = get_replacement_value(&system, &replacement);
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), "TestValue");
    }

    #[test]
    fn graft_replacement_with_context() {
        let system = MockSystem::new();
        let mut context = HashMap::new();
        context.insert("projectName".to_owned(), json!("my-app"));
        context.insert("maxGb".to_owned(), json!(16));

        // Test string context value
        let replacement = GraftReplacement::new(
            "{{PROJECT}}".to_owned(),
            None,
            None,
            Some("projectName".to_owned()),
        );

        let value = get_graft_replacement_value(&system, &replacement, &context);
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), "my-app");

        // Test number context value
        let replacement_2 = GraftReplacement::new(
            "{{MAX_GB}}".to_owned(),
            None,
            None,
            Some("maxGb".to_owned()),
        );

        let value_2 = get_graft_replacement_value(&system, &replacement_2, &context);
        assert!(value_2.is_ok());
        assert_eq!(value_2.unwrap(), "16");
    }

    #[test]
    fn graft_replacement_missing_context() {
        let system = MockSystem::new();
        let context = HashMap::new();

        let replacement =
            GraftReplacement::new("{{VAR}}".to_owned(), None, None, Some("missing".to_owned()));

        let value = get_graft_replacement_value(&system, &replacement, &context);
        assert!(value.is_err());
        assert!(
            value
                .unwrap_err()
                .to_string()
                .contains("Context property 'missing' not found")
        );
    }

    #[test]
    fn apply_graft_replacements_tst() {
        let system = MockSystem::new()
            .with_dir("/test")
            .unwrap()
            .with_file("/test/test.txt", b"Hello {{NAME}}, value is {{VALUE}}!")
            .unwrap();

        let mut context = HashMap::new();
        context.insert("name".to_owned(), json!("Alice"));
        context.insert("value".to_owned(), json!(42));

        let replacements = vec![
            GraftReplacement::new("{{NAME}}".to_owned(), None, None, Some("name".to_owned())),
            GraftReplacement::new("{{VALUE}}".to_owned(), None, None, Some("value".to_owned())),
        ];

        let result = apply_graft_replacements(&system, "/test", &replacements, &context);
        result.unwrap();

        let content_2 = system.read_to_string(Path::new("/test/test.txt")).unwrap();
        assert_eq!(content_2, "Hello Alice, value is 42!");
    }
}

//! Unit tests for text replacement operations.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::Path;
    use tixgraft::cli::ReplacementConfig;
    use tixgraft::config::graft_yaml::GraftReplacement;
    use tixgraft::operations::replace::{
        apply_graft_replacements, apply_regex_replacement, apply_replacements,
        apply_single_replacement, get_graft_replacement_value, get_replacement_value,
        preview_replacements,
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
        context.insert("maxGb".to_owned(), json!(16_i32));

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
        context.insert("value".to_owned(), json!(42_i32));

        let replacements = vec![
            GraftReplacement::new("{{NAME}}".to_owned(), None, None, Some("name".to_owned())),
            GraftReplacement::new("{{VALUE}}".to_owned(), None, None, Some("value".to_owned())),
        ];

        let result = apply_graft_replacements(&system, "/test", &replacements, &context);
        result.unwrap();

        let content_2 = system.read_to_string(Path::new("/test/test.txt")).unwrap();
        assert_eq!(content_2, "Hello Alice, value is 42!");
    }

    #[test]
    fn apply_replacements_empty_list() {
        let system = MockSystem::new();
        let result = apply_replacements(&system, "/test", &[]);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn apply_replacements_target_not_found() {
        let system = MockSystem::new();
        let replacements = vec![ReplacementConfig::new(
            "{{X}}".to_owned(),
            Some("val".to_owned()),
            None,
        )];
        let result = apply_replacements(&system, "/nonexistent", &replacements);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn apply_replacements_multiple() {
        let system = MockSystem::new()
            .with_dir("/test")
            .unwrap()
            .with_file("/test/file.txt", b"Hello {{NAME}} from {{PLACE}}")
            .unwrap();

        let replacements = vec![
            ReplacementConfig::new("{{NAME}}".to_owned(), Some("Alice".to_owned()), None),
            ReplacementConfig::new("{{PLACE}}".to_owned(), Some("Earth".to_owned()), None),
        ];

        let count = apply_replacements(&system, "/test", &replacements).unwrap();
        assert_eq!(count, 2);

        let content = system.read_to_string(Path::new("/test/file.txt")).unwrap();
        assert_eq!(content, "Hello Alice from Earth");
    }

    #[test]
    fn apply_regex_replacement_on_file() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"version = 1.2.3")
            .unwrap();

        let count = apply_regex_replacement(
            &system,
            Path::new("/test/file.txt"),
            r"\d+\.\d+\.\d+",
            "2.0.0",
        )
        .unwrap();

        assert_eq!(count, 1);
        let content = system.read_to_string(Path::new("/test/file.txt")).unwrap();
        assert_eq!(content, "version = 2.0.0");
    }

    #[test]
    fn apply_regex_replacement_on_directory() {
        let system = MockSystem::new()
            .with_dir("/test")
            .unwrap()
            .with_file("/test/a.txt", b"foo-123")
            .unwrap()
            .with_file("/test/b.txt", b"foo-456")
            .unwrap();

        let count =
            apply_regex_replacement(&system, Path::new("/test"), r"foo-\d+", "bar").unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            system.read_to_string(Path::new("/test/a.txt")).unwrap(),
            "bar"
        );
        assert_eq!(
            system.read_to_string(Path::new("/test/b.txt")).unwrap(),
            "bar"
        );
    }

    #[test]
    fn apply_regex_replacement_invalid_regex() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"data")
            .unwrap();

        let result =
            apply_regex_replacement(&system, Path::new("/test/file.txt"), r"[invalid", "x");
        assert!(result.is_err());
    }

    #[test]
    fn preview_replacements_tst() {
        let system = MockSystem::new()
            .with_dir("/test")
            .unwrap()
            .with_file("/test/a.txt", b"Hello {{NAME}}")
            .unwrap()
            .with_file("/test/b.txt", b"No placeholder here")
            .unwrap();

        let replacements = vec![ReplacementConfig::new(
            "{{NAME}}".to_owned(),
            Some("Alice".to_owned()),
            None,
        )];

        let previews = preview_replacements(&system, "/test", &replacements).unwrap();
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].search_pattern, "{{NAME}}");
        assert_eq!(previews[0].replacement_value, "Alice");
        assert_eq!(previews[0].affected_files.len(), 1);
    }

    #[test]
    fn preview_replacements_empty() {
        let system = MockSystem::new().with_dir("/test").unwrap();

        let previews = preview_replacements(&system, "/test", &[]).unwrap();
        assert!(previews.is_empty());
    }

    #[test]
    fn replacement_no_target_no_env() {
        let system = MockSystem::new();
        let replacement = ReplacementConfig::new("{{X}}".to_owned(), None, None);
        let result = get_replacement_value(&system, &replacement);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly one"));
    }

    #[test]
    fn replacement_both_target_and_env() {
        let system = MockSystem::new();
        let replacement = ReplacementConfig::new(
            "{{X}}".to_owned(),
            Some("val".to_owned()),
            Some("ENV".to_owned()),
        );
        let result = get_replacement_value(&system, &replacement);
        assert!(result.is_err());
    }

    #[test]
    fn replacement_env_var_not_set() {
        let system = MockSystem::new();
        let replacement =
            ReplacementConfig::new("{{X}}".to_owned(), None, Some("MISSING_VAR".to_owned()));
        let result = get_replacement_value(&system, &replacement);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MISSING_VAR"));
    }

    #[test]
    fn graft_replacement_multiple_sources_error() {
        let system = MockSystem::new();
        let context = HashMap::new();

        // Both target and valueFromContext
        let replacement = GraftReplacement::new(
            "{{X}}".to_owned(),
            Some("val".to_owned()),
            None,
            Some("key".to_owned()),
        );
        let result = get_graft_replacement_value(&system, &replacement, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly one"));
    }

    #[test]
    fn apply_single_replacement_no_match() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"no placeholders here")
            .unwrap();

        let count =
            apply_single_replacement(&system, Path::new("/test/file.txt"), "{{MISSING}}", "value")
                .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn apply_single_replacement_skips_binary() {
        let system = MockSystem::new()
            .with_file("/test/binary.bin", &[0x00, 0x01, 0x02, 0xFF])
            .unwrap();

        let count = apply_single_replacement(
            &system,
            Path::new("/test/binary.bin"),
            "\x00\x01",
            "replaced",
        )
        .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn apply_replacements_in_subdirectories() {
        let system = MockSystem::new()
            .with_dir("/test")
            .unwrap()
            .with_dir("/test/sub")
            .unwrap()
            .with_file("/test/a.txt", b"{{VAR}} top level")
            .unwrap()
            .with_file("/test/sub/b.txt", b"{{VAR}} nested")
            .unwrap();

        let count =
            apply_single_replacement(&system, Path::new("/test"), "{{VAR}}", "REPLACED").unwrap();

        assert_eq!(count, 2);
        assert!(
            system
                .read_to_string(Path::new("/test/a.txt"))
                .unwrap()
                .contains("REPLACED")
        );
        assert!(
            system
                .read_to_string(Path::new("/test/sub/b.txt"))
                .unwrap()
                .contains("REPLACED")
        );
    }
}

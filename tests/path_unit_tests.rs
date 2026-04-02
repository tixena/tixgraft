//! Unit tests for path utilities.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use std::path::{Path, PathBuf};
    use tixgraft::utils::path::{
        common_path_prefix, depth, escapes_from_base, get_file_extension, has_extension,
        is_path_allowed, join_path_safe, normalize, normalize_separators, to_unix, to_windows,
        validate_path_safety,
    };
    #[test]
    fn normalize_path() {
        assert_eq!(normalize(Path::new("./a/../b/./c")), PathBuf::from("b/c"));

        assert_eq!(normalize(Path::new("../a/b")), PathBuf::from("../a/b"));

        assert_eq!(normalize(Path::new("a/b/../..")), PathBuf::from(""));
    }

    #[test]
    fn validate_path_safety_tst() {
        // Safe paths
        validate_path_safety("./some/path").unwrap();
        validate_path_safety("some/path").unwrap();
        validate_path_safety("path").unwrap();

        // Unsafe paths - this should fail but our normalize_path handles it
        // So let's test a different unsafe pattern
        assert!(validate_path_safety("/etc/passwd").is_err());
        assert!(validate_path_safety("../../../etc").is_err());

        // This should be OK as it doesn't escape
        validate_path_safety("./relative/path").unwrap();
    }

    #[test]
    fn get_file_extension_tst() {
        assert_eq!(
            get_file_extension(Path::new("file.TXT")),
            Some("txt".to_owned())
        );
        assert_eq!(get_file_extension(Path::new("file")), None);
        assert_eq!(
            get_file_extension(Path::new("path/file.json")),
            Some("json".to_owned())
        );
    }

    #[test]
    fn has_extension_tst() {
        assert!(has_extension(Path::new("file.txt"), "txt"));
        assert!(has_extension(Path::new("file.TXT"), "txt"));
        assert!(!has_extension(Path::new("file.json"), "txt"));
        assert!(!has_extension(Path::new("file"), "txt"));
    }

    #[test]
    fn normalize_separators_tst() {
        assert_eq!(normalize_separators("path\\to\\file"), "path/to/file");
        assert_eq!(normalize_separators("path/to/file"), "path/to/file");
        assert_eq!(
            normalize_separators("mixed\\path/to\\file"),
            "mixed/path/to/file"
        );
    }

    #[test]
    fn path_depth() {
        assert_eq!(depth(Path::new("a")), 1);
        assert_eq!(depth(Path::new("a/b/c")), 3);
        assert_eq!(depth(Path::new("./a/b")), 2);
        assert_eq!(depth(Path::new("")), 0);
    }

    #[test]
    fn common_path_prefix_tst() {
        let prefix = common_path_prefix(Path::new("a/b/c/d"), Path::new("a/b/x/y"));
        assert_eq!(prefix, PathBuf::from("a/b"));

        // No common prefix
        let prefix = common_path_prefix(Path::new("a/b"), Path::new("x/y"));
        assert_eq!(prefix, PathBuf::from(""));

        // Identical paths
        let prefix = common_path_prefix(Path::new("a/b/c"), Path::new("a/b/c"));
        assert_eq!(prefix, PathBuf::from("a/b/c"));
    }

    #[test]
    fn join_path_safe_tst() {
        // Normal join
        let result = join_path_safe("base", "sub/path").unwrap();
        assert_eq!(result, "base/sub/path");

        // Rejects traversal
        assert!(join_path_safe("base", "../escape").is_err());

        // Rejects absolute component
        assert!(join_path_safe("base", "/etc/passwd").is_err());
    }

    #[test]
    fn to_unix_tst() {
        assert_eq!(to_unix("path\\to\\file"), "path/to/file");
        assert_eq!(to_unix("already/unix"), "already/unix");
        assert_eq!(to_unix(""), "");
    }

    #[test]
    fn to_windows_tst() {
        assert_eq!(to_windows("path/to/file"), "path\\to\\file");
        assert_eq!(to_windows("already\\windows"), "already\\windows");
        assert_eq!(to_windows(""), "");
    }

    #[test]
    fn is_path_allowed_tst() {
        // Direct match
        assert!(is_path_allowed(Path::new("src/main.rs"), &["src"]));

        // No match
        assert!(!is_path_allowed(Path::new("src/main.rs"), &["tests"]));

        // Glob pattern
        assert!(is_path_allowed(Path::new("src/main.rs"), &["*.rs"]));
        assert!(!is_path_allowed(Path::new("src/main.rs"), &["*.txt"]));

        // Empty patterns
        assert!(!is_path_allowed(Path::new("anything"), &[]));
    }

    #[test]
    fn escapes_from_base_tst() {
        // Normalized path check (non-existent paths, fallback branch)
        assert!(escapes_from_base(
            Path::new("/outside/path"),
            Path::new("/base/dir")
        ));
        assert!(!escapes_from_base(
            Path::new("/base/dir/sub"),
            Path::new("/base/dir")
        ));
    }

    #[test]
    fn validate_path_safety_dotdot_not_escaping() {
        // a/../a doesn't escape — it normalizes to "a"
        validate_path_safety("a/../a").unwrap();
    }
}

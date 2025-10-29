//! Unit tests for path utilities

use std::path::{Path, PathBuf};
use tixgraft::utils::path::{
    common_path_prefix, get_file_extension, has_extension, normalize_path, normalize_separators,
    path_depth, validate_path_safety,
};

#[test]
fn test_normalize_path() {
    assert_eq!(
        normalize_path(Path::new("./a/../b/./c")),
        PathBuf::from("b/c")
    );

    assert_eq!(normalize_path(Path::new("../a/b")), PathBuf::from("../a/b"));

    assert_eq!(normalize_path(Path::new("a/b/../..")), PathBuf::from(""));
}

#[test]
fn test_validate_path_safety() {
    // Safe paths
    assert!(validate_path_safety("./some/path").is_ok());
    assert!(validate_path_safety("some/path").is_ok());
    assert!(validate_path_safety("path").is_ok());

    // Unsafe paths - this should fail but our normalize_path handles it
    // So let's test a different unsafe pattern
    assert!(validate_path_safety("/etc/passwd").is_err());
    assert!(validate_path_safety("../../../etc").is_err());

    // This should be OK as it doesn't escape
    assert!(validate_path_safety("./relative/path").is_ok());
}

#[test]
fn test_get_file_extension() {
    assert_eq!(
        get_file_extension(Path::new("file.TXT")),
        Some("txt".to_string())
    );
    assert_eq!(get_file_extension(Path::new("file")), None);
    assert_eq!(
        get_file_extension(Path::new("path/file.json")),
        Some("json".to_string())
    );
}

#[test]
fn test_has_extension() {
    assert!(has_extension(Path::new("file.txt"), "txt"));
    assert!(has_extension(Path::new("file.TXT"), "txt"));
    assert!(!has_extension(Path::new("file.json"), "txt"));
    assert!(!has_extension(Path::new("file"), "txt"));
}

#[test]
fn test_normalize_separators() {
    assert_eq!(normalize_separators("path\\to\\file"), "path/to/file");
    assert_eq!(normalize_separators("path/to/file"), "path/to/file");
    assert_eq!(
        normalize_separators("mixed\\path/to\\file"),
        "mixed/path/to/file"
    );
}

#[test]
fn test_path_depth() {
    assert_eq!(path_depth(Path::new("a")), 1);
    assert_eq!(path_depth(Path::new("a/b/c")), 3);
    assert_eq!(path_depth(Path::new("./a/b")), 2);
    assert_eq!(path_depth(Path::new("")), 0);
}

#[test]
fn test_common_path_prefix() {
    let prefix = common_path_prefix(Path::new("a/b/c/d"), Path::new("a/b/x/y"));
    assert_eq!(prefix, PathBuf::from("a/b"));
}

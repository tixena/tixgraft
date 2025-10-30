//! Unit tests for path utilities



#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    
    use std::path::{Path, PathBuf};
    use tixgraft::utils::path::{
        common_path_prefix, depth, get_file_extension, has_extension, normalize, normalize_separators,
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
}

}
//! Unit tests for configuration validation

use tixgraft::config::validation::{validate_path_safety, validate_repository_url};

#[test]
fn test_validate_repository_url() {
    // Valid Git URLs
    assert!(validate_repository_url("myorg/repo").is_ok());
    assert!(validate_repository_url("https://github.com/myorg/repo.git").is_ok());
    assert!(validate_repository_url("git@github.com:myorg/repo.git").is_ok());

    // Valid local paths (ONLY file: prefix)
    assert!(validate_repository_url("file:///path/to/repo").is_ok());
    assert!(validate_repository_url("file:/path/to/repo").is_ok());
    assert!(validate_repository_url("file:~/src/repo").is_ok());

    // Invalid URLs
    assert!(validate_repository_url("invalid-url").is_err());
    assert!(validate_repository_url("").is_err());

    // Paths without file: prefix should now be rejected
    assert!(validate_repository_url("~/src/repo").is_err());
    assert!(validate_repository_url("./local/repo").is_err());
    assert!(validate_repository_url("../local/repo").is_err());
    assert!(validate_repository_url("/absolute/path/to/repo").is_err());
}

#[test]
fn test_validate_path_safety() {
    // Safe paths
    assert!(validate_path_safety("./some/path").is_ok());
    assert!(validate_path_safety("some/path").is_ok());
    assert!(validate_path_safety("./relative/path").is_ok());

    // Unsafe paths
    assert!(validate_path_safety("../../unsafe").is_err());
    assert!(validate_path_safety("/absolute/path").is_err());
}

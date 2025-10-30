//! Unit tests for configuration validation



#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
use tixgraft::config::validation::{validate_path_safety, validate_repository_url};

#[test]
fn validate_repository_url_tst() {
    // Valid Git URLs
    validate_repository_url("my_organization/repo").unwrap();
    validate_repository_url("https://github.com/my_organization/repo.git").unwrap();
    validate_repository_url("git@github.com:my_organization/repo.git").unwrap();

    // Valid local paths (ONLY file: prefix)
    validate_repository_url("file:///path/to/repo").unwrap();
    validate_repository_url("file:/path/to/repo").unwrap();
    validate_repository_url("file:~/src/repo").unwrap();

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
fn validate_path_safety_tst() {
    // Safe paths
    validate_path_safety("./some/path").unwrap();
    validate_path_safety("some/path").unwrap();
    validate_path_safety("./relative/path").unwrap();

    // Unsafe paths
    assert!(validate_path_safety("../../unsafe").is_err());
    assert!(validate_path_safety("/absolute/path").is_err());
}
}
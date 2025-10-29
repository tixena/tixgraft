//! Unit tests for Git sparse checkout utilities

use tixgraft::git::sparse_checkout::parse_git_version;

#[test]
fn test_parse_git_version() {
    assert_eq!(parse_git_version("2.34.1").unwrap(), (2, 34, 1));
    assert_eq!(parse_git_version("2.25.0").unwrap(), (2, 25, 0));
    assert!(parse_git_version("invalid").is_err());
}

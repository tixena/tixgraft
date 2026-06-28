#![expect(clippy::unwrap_used, reason = "These are unit tests")]

use super::*;

#[test]
fn normalize_repository_url_tst() {
    assert_eq!(
        normalize_repository_url("my_organization/repo").unwrap(),
        "https://github.com/my_organization/repo.git"
    );
    assert_eq!(
        normalize_repository_url("https://github.com/my_organization/repo").unwrap(),
        "https://github.com/my_organization/repo.git"
    );
    assert_eq!(
        normalize_repository_url("https://github.com/my_organization/repo.git").unwrap(),
        "https://github.com/my_organization/repo.git"
    );
    assert_eq!(
        normalize_repository_url("git@github.com:my_organization/repo.git").unwrap(),
        "git@github.com:my_organization/repo.git"
    );
}

#[test]
fn invalid_repository_urls() {
    normalize_repository_url("invalid").unwrap_err();
    normalize_repository_url("").unwrap_err();
    normalize_repository_url("too/many/slashes").unwrap_err();
}

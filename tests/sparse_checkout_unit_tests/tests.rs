#![expect(clippy::unwrap_used, reason = "This is a test module")]

use tixgraft::git::sparse_checkout::check_git_support;

#[test]
fn check_git_support_accepts_the_linked_libgit2() {
    check_git_support().unwrap();
}

#[test]
fn linked_libgit2_carries_both_transports() {
    let version = git2::Version::get();

    assert!(version.https(), "libgit2 must be built with HTTPS support");
    assert!(version.ssh(), "libgit2 must be built with SSH support");
}

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::mock::MockSystem;
use tixgraft::git::Repository;

#[test]
fn detect_git_source() {
    let system = MockSystem::new();

    let repo = Repository::new(&system, "my_organization/repo").unwrap();
    assert!(repo.is_git());
    assert!(!repo.is_local());
    assert_eq!(
        repo.git_url().unwrap(),
        "https://github.com/my_organization/repo.git"
    );

    let repo_2 = Repository::new(&system, "https://github.com/my_organization/repo.git").unwrap();
    assert!(repo_2.is_git());
    assert!(!repo_2.is_local());

    let repo_3 = Repository::new(&system, "git@github.com:my_organization/repo.git").unwrap();
    assert!(repo_3.is_git());
    assert!(!repo_3.is_local());
}

#[test]
fn detect_local_source_with_file_prefix() {
    let system = MockSystem::new().with_dir("/test/local_repo").unwrap();

    let repo = Repository::new(&system, "file:///test/local_repo").unwrap();

    assert!(repo.is_local());
    assert!(!repo.is_git());
    assert!(repo.local_path().is_some());
    assert_eq!(
        repo.local_path().unwrap().to_str().unwrap(),
        "/test/local_repo"
    );
}

#[test]
fn detect_local_source_with_absolute_path() {
    let system = MockSystem::new().with_dir("/test/abs_repo").unwrap();

    let repo = Repository::new(&system, "file:/test/abs_repo").unwrap();

    assert!(repo.is_local());
    assert!(!repo.is_git());
    assert!(repo.local_path().is_some());
}

#[test]
fn detect_local_source_with_relative_path() {
    let system = MockSystem::new()
        .with_current_dir("/work")
        .unwrap()
        .with_dir("/work/my_repo")
        .unwrap();

    let repo = Repository::new(&system, "file:./my_repo").unwrap();

    assert!(repo.is_local());
    assert!(!repo.is_git());
    assert!(repo.local_path().is_some());
}

#[test]
fn local_source_nonexistent_path() {
    let system = MockSystem::new();
    let err = Repository::new(&system, "file:///nonexistent/path/that/does/not/exist").unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn local_source_file_not_directory() {
    let system = MockSystem::new()
        .with_file("/test/test.txt", b"test")
        .unwrap();

    let err = Repository::new(&system, "file:///test/test.txt").unwrap_err();
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn repository_methods() {
    let system = MockSystem::new().with_dir("/test/local_repo").unwrap();

    let repo = Repository::new(&system, "my_organization/repo").unwrap();
    assert_eq!(repo.original_url(), "my_organization/repo");
    assert_eq!(
        repo.git_url().unwrap(),
        "https://github.com/my_organization/repo.git"
    );
    assert_eq!(repo.local_path(), None);

    let new_repo = Repository::new(&system, "file:///test/local_repo").unwrap();
    assert_eq!(new_repo.original_url(), "file:///test/local_repo");
    assert!(new_repo.local_path().is_some());
    assert_eq!(
        new_repo.local_path().unwrap().to_str().unwrap(),
        "/test/local_repo"
    );
}

//! Unit tests for `Repository` public API.
//!
//! Private function tests (`normalize_repository_url`) remain inline in `src/git/repository.rs`.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use os_shim::System as _;
    use os_shim::real::RealSystem;
    use tempfile::TempDir;
    use tixgraft::git::Repository;

    #[test]
    fn detect_git_source() {
        let system = RealSystem::new();

        let repo = Repository::new(&system, "my_organization/repo").unwrap();
        assert!(repo.is_git());
        assert!(!repo.is_local());
        assert_eq!(
            repo.git_url().unwrap(),
            "https://github.com/my_organization/repo.git"
        );

        let repo_2 =
            Repository::new(&system, "https://github.com/my_organization/repo.git").unwrap();
        assert!(repo_2.is_git());
        assert!(!repo_2.is_local());

        let repo_3 = Repository::new(&system, "git@github.com:my_organization/repo.git").unwrap();
        assert!(repo_3.is_git());
        assert!(!repo_3.is_local());
    }

    #[test]
    fn detect_local_source_with_file_prefix() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let url = format!("file://{}", path.display());
        let repo = Repository::new(&system, &url).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
        assert_eq!(repo.local_path().unwrap(), path);
    }

    #[test]
    fn detect_local_source_with_absolute_path() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        let url = format!("file:{path}");

        let repo = Repository::new(&system, &url).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn detect_local_source_with_relative_path() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new_in(".").unwrap();
        let dir_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
        let relative_path = format!("file:./{dir_name}");

        let repo = Repository::new(&system, &relative_path).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn local_source_nonexistent_path() {
        let system = RealSystem::new();
        let err =
            Repository::new(&system, "file:///nonexistent/path/that/does/not/exist").unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn local_source_file_not_directory() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        system.write(&file_path, b"test").unwrap();

        let url = format!("file://{}", file_path.display());
        let err = Repository::new(&system, &url).unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn repository_methods() {
        let system = RealSystem::new();

        let repo = Repository::new(&system, "my_organization/repo").unwrap();
        assert_eq!(repo.original_url(), "my_organization/repo");
        assert_eq!(
            repo.git_url().unwrap(),
            "https://github.com/my_organization/repo.git"
        );
        assert_eq!(repo.local_path(), None);

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let url = format!("file://{}", path.display());
        let new_repo = Repository::new(&system, &url).unwrap();

        assert_eq!(new_repo.original_url(), &url);
        assert!(new_repo.local_path().is_some());
        assert_eq!(new_repo.local_path().unwrap(), path);
    }
}

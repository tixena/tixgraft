//! Tests for System temp directory abstraction


#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

use tixgraft::system::mock::MockSystem;
use tixgraft::system::real::RealSystem;
use tixgraft::system::System as _;

#[test]
fn mock_temp_dir_creation() {
    let system = MockSystem::new();

    let temp_dir = system.create_temp_dir().unwrap();
    let temp_path = temp_dir.path();

    // Verify the temp directory exists
    assert!(system.exists(temp_path).unwrap());
    assert!(system.is_dir(temp_path).unwrap());

    // Create a file in the temp directory
    let file_path = temp_path.join("test.txt");
    system.write(&file_path, b"test content").unwrap();
    assert!(system.exists(&file_path).unwrap());
}

#[test]
fn mock_temp_dir_cleanup_on_drop() {
    let system = MockSystem::new();
    let temp_path = {
        let temp_dir = system.create_temp_dir().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Create files in the temp directory
        system.write(&path.join("file1.txt"), b"content1").unwrap();
        system.write(&path.join("file2.txt"), b"content2").unwrap();

        // Verify files exist
        assert!(system.exists(&path.join("file1.txt")).unwrap());
        assert!(system.exists(&path.join("file2.txt")).unwrap());

        path
        // temp_dir is dropped here
    };

    // After drop, temp directory and its contents should be gone
    assert!(!system.exists(&temp_path).unwrap());
    assert!(!system.exists(&temp_path.join("file1.txt")).unwrap());
    assert!(!system.exists(&temp_path.join("file2.txt")).unwrap());
}

#[test]
fn mock_multiple_temp_dirs() {
    let system = MockSystem::new();

    let temp1 = system.create_temp_dir().unwrap();
    let temp2 = system.create_temp_dir().unwrap();
    let temp3 = system.create_temp_dir().unwrap();

    // All temp directories should have unique paths
    assert_ne!(temp1.path(), temp2.path());
    assert_ne!(temp2.path(), temp3.path());
    assert_ne!(temp1.path(), temp3.path());

    // All should exist
    assert!(system.exists(temp1.path()).unwrap());
    assert!(system.exists(temp2.path()).unwrap());
    assert!(system.exists(temp3.path()).unwrap());
}

#[test]
fn real_temp_dir_creation() {
    let system = RealSystem::new();

    let temp_dir = system.create_temp_dir().unwrap();
    let temp_path = temp_dir.path();

    // Verify the temp directory exists on real filesystem
    assert!(temp_path.exists());
    assert!(temp_path.is_dir());

    // Create a file in the temp directory
    let file_path = temp_path.join("test.txt");
    std::fs::write(&file_path, b"test content").unwrap();
    assert!(file_path.exists());
}

#[test]
fn real_temp_dir_cleanup_on_drop() {
    let system = RealSystem::new();
    let temp_path = {
        let temp_dir = system.create_temp_dir().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Create files in the temp directory
        std::fs::write(path.join("file1.txt"), b"content1").unwrap();
        std::fs::write(path.join("file2.txt"), b"content2").unwrap();

        // Verify files exist
        assert!(path.join("file1.txt").exists());
        assert!(path.join("file2.txt").exists());

        path
        // temp_dir is dropped here
    };

    // After drop, temp directory and its contents should be gone
    assert!(!temp_path.exists());
}

#[test]
fn temp_dir_with_subdirectories() {
    let system = MockSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    let temp_path = temp_dir.path();

    // Create nested directory structure
    let subdir = temp_path.join("subdir");
    system.create_dir_all(&subdir).unwrap();
    system
        .write(&subdir.join("nested.txt"), b"nested content")
        .unwrap();

    assert!(system.exists(&subdir).unwrap());
    assert!(system.exists(&subdir.join("nested.txt")).unwrap());

    // Keep temp_path for verification after drop
    let path_copy = temp_path.to_path_buf();
    drop(temp_dir);

    // Everything should be cleaned up
    assert!(!system.exists(&path_copy).unwrap());
    assert!(!system.exists(&path_copy.join("subdir")).unwrap());
}
}

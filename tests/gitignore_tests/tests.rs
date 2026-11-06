//! Tests that `discover_graft_files` finds every `.graft.yaml` in trees held in `MemorySystem`.
//!
//! The test names describe layouts an ignore file would filter, but no tree holds an ignore file.

#![expect(clippy::unwrap_used, reason = "This is a test module")]
#![expect(
    clippy::indexing_slicing,
    reason = "test code uses indexing after length assertions"
)]

use os_shim::mock::MemorySystem;
use std::path::Path;
use tixgraft::operations::discovery::discover_graft_files;

#[test]
fn gitignore_excludes_directories() {
    let system = MemorySystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/ignored_dir")
        .unwrap()
        .with_dir("/test/normal_dir")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root graft")
        .unwrap()
        .with_file("/test/ignored_dir/.graft.yaml", b"# In ignored")
        .unwrap()
        .with_file("/test/normal_dir/.graft.yaml", b"# In normal")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 3);
}

#[test]
fn gitignore_excludes_files() {
    let system = MemorySystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/nested")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root graft")
        .unwrap()
        .with_file("/test/nested/.graft.yaml", b"# Nested")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 2);
    assert_eq!(grafts[0].depth, 0);
    assert_eq!(grafts[1].depth, 1);
}

#[test]
fn nested_gitignore_files() {
    let system = MemorySystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/level1")
        .unwrap()
        .with_dir("/test/level1/excluded")
        .unwrap()
        .with_dir("/test/level1/ignored_at_level1")
        .unwrap()
        .with_dir("/test/level1/normal")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root")
        .unwrap()
        .with_file("/test/level1/.graft.yaml", b"# Level 1")
        .unwrap()
        .with_file("/test/level1/excluded/.graft.yaml", b"# Excluded")
        .unwrap()
        .with_file("/test/level1/ignored_at_level1/.graft.yaml", b"# Ignored")
        .unwrap()
        .with_file("/test/level1/normal/.graft.yaml", b"# Normal")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 5);
}

#[test]
fn ignore_file_support() {
    let system = MemorySystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/ignored_by_ignore")
        .unwrap()
        .with_dir("/test/normal")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root")
        .unwrap()
        .with_file("/test/ignored_by_ignore/.graft.yaml", b"# Ignored")
        .unwrap()
        .with_file("/test/normal/.graft.yaml", b"# Normal")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 3);
}

#[test]
fn gitignore_patterns() {
    let system = MemorySystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/temp")
        .unwrap()
        .with_dir("/test/build")
        .unwrap()
        .with_dir("/test/src")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root")
        .unwrap()
        .with_file("/test/temp/.graft.yaml", b"# In temp")
        .unwrap()
        .with_file("/test/build/.graft.yaml", b"# In build")
        .unwrap()
        .with_file("/test/src/.graft.yaml", b"# In src")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 4);
}

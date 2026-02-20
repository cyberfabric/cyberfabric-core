#![allow(clippy::unwrap_used, clippy::expect_used, clippy::use_debug)]

use std::path::PathBuf;
use std::sync::Arc;

use file_parser::domain::error::DomainError;
use file_parser::domain::parser::FileParserBackend;
use file_parser::domain::service::{FileParserService, ServiceConfig};
use file_parser::infra::parsers::PlainTextParser;

/// Build a minimal `FileParserService` with the given base-dir restriction.
fn build_service(allowed_local_base_dir: PathBuf) -> FileParserService {
    let parsers: Vec<Arc<dyn FileParserBackend>> = vec![Arc::new(PlainTextParser::new())];
    let config = ServiceConfig {
        max_file_size_bytes: 10 * 1024 * 1024,
        allowed_local_base_dir,
    };

    FileParserService::new(parsers, config)
}

/// Create a temporary text file inside the given directory and return its path.
fn create_temp_file(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("failed to create temp file");
    path
}

// -----------------------------------------------------------------------
// 1. `..` component rejection
// -----------------------------------------------------------------------

#[tokio::test]
async fn rejects_dotdot_relative_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let svc = build_service(tmp.path().canonicalize().unwrap());
    let path = PathBuf::from("some/../../etc/passwd");

    let err = svc.parse_local(&path).await.unwrap_err();
    assert!(
        matches!(err, DomainError::PathTraversalBlocked { .. }),
        "Expected PathTraversalBlocked, got: {err:?}"
    );
}

#[tokio::test]
async fn rejects_dotdot_at_start() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let svc = build_service(tmp.path().canonicalize().unwrap());
    let path = PathBuf::from("../secret.txt");

    let err = svc.parse_local(&path).await.unwrap_err();
    assert!(matches!(err, DomainError::PathTraversalBlocked { .. }));
}

#[tokio::test]
async fn rejects_dotdot_in_middle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let svc = build_service(tmp.path().canonicalize().unwrap());
    let path = PathBuf::from("/allowed/dir/../../../etc/shadow");

    let err = svc.parse_local(&path).await.unwrap_err();
    assert!(matches!(err, DomainError::PathTraversalBlocked { .. }));
}

// -----------------------------------------------------------------------
// 2. Base-dir enforcement
// -----------------------------------------------------------------------

#[tokio::test]
async fn allows_file_within_base_dir() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let base = tmp.path().canonicalize().unwrap();
    let file = create_temp_file(&base, "hello.txt", "Hello, world!");

    let svc = build_service(base);
    let doc = svc.parse_local(&file).await.expect("should parse OK");
    assert!(!doc.blocks.is_empty(), "should produce blocks");
}

#[tokio::test]
async fn allows_file_in_subdirectory_of_base_dir() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let base = tmp.path().canonicalize().unwrap();
    let sub = base.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    let file = create_temp_file(&sub, "nested.txt", "Nested content");

    let svc = build_service(base);
    let doc = svc.parse_local(&file).await.expect("should parse OK");
    assert!(!doc.blocks.is_empty());
}

#[tokio::test]
async fn rejects_file_outside_base_dir() {
    let base_tmp = tempfile::tempdir().expect("failed to create base dir");
    let other_tmp = tempfile::tempdir().expect("failed to create other dir");

    let base = base_tmp.path().canonicalize().unwrap();
    let outside_file = create_temp_file(other_tmp.path(), "secret.txt", "Secret data");

    let svc = build_service(base);
    let err = svc.parse_local(&outside_file).await.unwrap_err();
    assert!(
        matches!(err, DomainError::PathTraversalBlocked { .. }),
        "Expected PathTraversalBlocked, got: {err:?}"
    );
}

#[tokio::test]
async fn rejects_absolute_path_outside_base_dir() {
    let base_tmp = tempfile::tempdir().expect("failed to create base dir");
    let base = base_tmp.path().canonicalize().unwrap();

    let other_tmp = tempfile::tempdir().expect("failed to create other dir");
    let outside = create_temp_file(other_tmp.path(), "data.log", "log line");

    let svc = build_service(base);
    let err = svc.parse_local(&outside).await.unwrap_err();
    assert!(matches!(err, DomainError::PathTraversalBlocked { .. }));
}

// -----------------------------------------------------------------------
// 3. Symlink escape prevention
// -----------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlink_escape_from_base_dir() {
    let base_tmp = tempfile::tempdir().expect("failed to create base dir");
    let external_tmp = tempfile::tempdir().expect("failed to create external dir");

    let base = base_tmp.path().canonicalize().unwrap();
    let external_file = create_temp_file(external_tmp.path(), "secret.txt", "Confidential content");

    // Create a symlink inside the base dir that points outside
    let symlink_path = base.join("escape.txt");
    std::os::unix::fs::symlink(&external_file, &symlink_path).expect("failed to create symlink");

    let svc = build_service(base);
    let err = svc.parse_local(&symlink_path).await.unwrap_err();
    assert!(
        matches!(err, DomainError::PathTraversalBlocked { .. }),
        "Symlink escaping base dir should be blocked, got: {err:?}"
    );
}

// -----------------------------------------------------------------------
// 4. Edge cases
// -----------------------------------------------------------------------

#[tokio::test]
async fn file_not_found_still_works() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().canonicalize().unwrap();
    let svc = build_service(base.clone());
    // Use a path inside the base dir that doesn't exist
    let path = base.join("nonexistent.txt");

    let err = svc.parse_local(&path).await.unwrap_err();
    assert!(
        matches!(err, DomainError::FileNotFound { .. }),
        "Expected FileNotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn dotdot_error_message_contains_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let svc = build_service(tmp.path().canonicalize().unwrap());
    let path = PathBuf::from("/safe/../etc/passwd");

    let err = svc.parse_local(&path).await.unwrap_err();
    match err {
        DomainError::PathTraversalBlocked { message } => {
            assert!(
                message.contains(".."),
                "Error message should mention '..': {message}"
            );
        }
        other => panic!("Expected PathTraversalBlocked, got: {other:?}"),
    }
}

#[tokio::test]
async fn base_dir_error_message_hides_canonical_path() {
    let base_tmp = tempfile::tempdir().expect("failed to create base dir");
    let other_tmp = tempfile::tempdir().expect("failed to create other dir");

    let base = base_tmp.path().canonicalize().unwrap();
    let outside = create_temp_file(other_tmp.path(), "leak.txt", "data");

    let svc = build_service(base.clone());
    let err = svc.parse_local(&outside).await.unwrap_err();
    match err {
        DomainError::PathTraversalBlocked { message } => {
            // The error message should NOT leak the base dir path to the caller
            assert!(
                !message.contains(&base.display().to_string()),
                "Error message should not reveal base dir: {message}"
            );
        }
        other => panic!("Expected PathTraversalBlocked, got: {other:?}"),
    }
}

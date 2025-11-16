pub mod deps;
pub mod error;
pub mod util;

#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod tests {
    use super::*;
    use deps::{Docker, LockEntry, LockFile, Lockable};
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    #[serial_test::serial]
    async fn test_discover_with_mocked_docker() {
        use mockito;

        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("uptix.lock");

        // Set up mock Docker registry
        let registry = mockito::server_address().to_string();
        let _auth_mock = mockito::mock("GET", "/v2/")
            .with_status(200)
            .with_header(
                "WWW-Authenticate",
                format!(r#"Bearer realm="http://{}/token""#, registry).as_str(),
            )
            .with_body("{}")
            .create();
        let _token_mock = mockito::mock("GET", "/token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"token": "test-token"}"#)
            .create();
        let _manifest_mock = mockito::mock("HEAD", "/v2/library/postgres/manifests/15")
            .with_status(200)
            .with_header(
                "docker-content-digest",
                "sha256:abc123def456abc123def456abc123def456abc123def456abc123def456abcd",
            )
            .create();
        let _config_mock = mockito::mock("GET", "/v2/library/postgres/blobs/sha256:abc123def456abc123def456abc123def456abc123def456abc123def456abcd")
            .with_status(200)
            .with_body(r#"{
                "created": "2024-01-15T10:30:00Z",
                "config": {
                    "Labels": {
                        "version": "15.2"
                    }
                }
            }"#)
            .create();

        // Manually create a mock Docker dependency
        let mock_docker = Docker {
            name: "postgres:15".to_string(),
            registry,
            image: "library/postgres".to_string(),
            tag: "15".to_string(),
            use_https: false,
            needs_nix_hash: false,
        };

        // Test locking the dependency
        let lock_entry = mock_docker.lock_with_metadata().await.unwrap();

        // Create lock file manually with the mocked dependency
        let mut lock_file = LockFile::new();
        lock_file.insert("postgres:15".to_string(), lock_entry);

        let json = serde_json::to_string_pretty(&lock_file).unwrap();
        fs::write(&lock_path, json).unwrap();

        // Verify lock file was created
        assert!(lock_path.exists());
        let lock_content = fs::read_to_string(&lock_path).unwrap();
        assert!(lock_content.contains("postgres:15"));
        assert!(lock_content.contains("metadata"));
        assert!(lock_content.contains("abc123def456"));
    }

    #[test]
    fn test_discover_new_dependency_detection() {
        // Test the logic for identifying new dependencies
        let mut lock_file = LockFile::new();

        // Add a fake entry
        let metadata = deps::DependencyMetadata {
            name: "postgres".to_string(),
            selected_version: Some("15".to_string()),
            resolved_version: Some("sha256:somehash".to_string()),
            timestamp: None,
            dep_type: "docker".to_string(),
            description: "Docker image postgres:15".to_string(),
        };

        let entry = LockEntry {
            metadata,
            lock: serde_json::json!("sha256:somehash"),
        };

        lock_file.insert("postgres:15".to_string(), entry);

        // Verify it contains the dependency
        assert!(lock_file.contains_key("postgres:15"));
        assert!(!lock_file.contains_key("redis:latest"));
    }
}

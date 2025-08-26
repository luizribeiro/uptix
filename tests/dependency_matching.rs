// Integration tests for dependency pattern matching functionality
// These tests verify the pattern matching logic across different dependency types

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn uptix_binary() -> String {
    // Get the path to the built uptix binary
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("uptix");
    path.to_str().unwrap().to_string()
}

#[test]
fn test_docker_pattern_update() {
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with Docker dependencies
    let nix_content = r#"{
        postgres = uptix.dockerImage "postgres:15";
        redis = uptix.dockerImage "redis:latest";
        ha = uptix.dockerImage "homeassistant/home-assistant:stable";
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Test updating just postgres without tag
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "postgres"])
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'postgres'"));
}

#[test]
fn test_github_release_pattern_update() {
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with GitHub release dependencies
    let nix_content = r#"{
        uptix = uptix.githubRelease {
            owner = "luizribeiro";
            repo = "uptix";
        };
        other = uptix.githubRelease {
            owner = "other";
            repo = "project";
        };
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Test updating using owner/repo pattern
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "luizribeiro/uptix"])
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'luizribeiro/uptix'"));
}

#[test]
fn test_github_branch_pattern_update() {
    use std::env;
    
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with GitHub branch dependencies
    let nix_content = r#"{
        uptixMain = uptix.githubBranch {
            owner = "luizribeiro";
            repo = "hello-world-rs";
            branch = "main";
        };
        uptixDev = uptix.githubBranch {
            owner = "luizribeiro";
            repo = "hello-world-rs";
            branch = "develop";
        };
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Create a mock lock file with existing entries to prevent actual API calls
    let lock_content = r#"{
        "$GITHUB_BRANCH$:luizribeiro/hello-world-rs:main$": {
            "metadata": {
                "name": "luizribeiro/hello-world-rs",
                "selected_version": "main",
                "resolved_version": "abc123",
                "friendly_version": "abc123",
                "dep_type": "github-branch",
                "description": "GitHub branch main from luizribeiro/hello-world-rs"
            },
            "lock": {
                "owner": "luizribeiro",
                "repo": "hello-world-rs",
                "rev": "abc123",
                "sha256": "0000000000000000000000000000000000000000000000000000",
                "fetchSubmodules": false,
                "deepClone": false,
                "leaveDotGit": false
            }
        },
        "$GITHUB_BRANCH$:luizribeiro/hello-world-rs:develop$": {
            "metadata": {
                "name": "luizribeiro/hello-world-rs",
                "selected_version": "develop", 
                "resolved_version": "def456",
                "friendly_version": "def456",
                "dep_type": "github-branch",
                "description": "GitHub branch develop from luizribeiro/hello-world-rs"
            },
            "lock": {
                "owner": "luizribeiro",
                "repo": "hello-world-rs",
                "rev": "def456",
                "sha256": "0000000000000000000000000000000000000000000000000000",
                "fetchSubmodules": false,
                "deepClone": false,
                "leaveDotGit": false
            }
        }
    }"#;
    fs::write(temp_dir.path().join("uptix.lock"), lock_content).unwrap();

    // Set a fake GitHub token to avoid rate limiting issues in CI
    env::set_var("GITHUB_TOKEN", "test-token");

    // Test updating using owner/repo:branch pattern
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "luizribeiro/hello-world-rs:main"])
        .output()
        .expect("Failed to execute uptix");

    // Clean up
    env::remove_var("GITHUB_TOKEN");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'luizribeiro/hello-world-rs:main'"));
}

#[test]
fn test_ambiguous_pattern_handling() {
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with both release and branch for same repo
    let nix_content = r#"{
        uptixRelease = uptix.githubRelease {
            owner = "luizribeiro";
            repo = "uptix";
        };
        uptixBranch = uptix.githubBranch {
            owner = "luizribeiro";
            repo = "uptix";
            branch = "main";
        };
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Test that owner/repo pattern only matches release
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "luizribeiro/uptix"])
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'luizribeiro/uptix'"));
}

#[test]
fn test_nonexistent_dependency_pattern() {
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with some dependencies
    let nix_content = r#"{
        postgres = uptix.dockerImage "postgres:15";
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Test updating non-existent dependency
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "mysql:8"])
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Dependency 'mysql:8' not found"));
}

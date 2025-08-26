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
    let temp_dir = TempDir::new().unwrap();

    // Create nix file with GitHub branch dependencies
    let nix_content = r#"{
        uptixMain = uptix.githubBranch {
            owner = "luizribeiro";
            repo = "uptix";
            branch = "main";
        };
        uptixDev = uptix.githubBranch {
            owner = "luizribeiro";
            repo = "uptix";
            branch = "develop";
        };
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Test updating using owner/repo:branch pattern
    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "luizribeiro/uptix:main"])
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'luizribeiro/uptix:main'"));
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

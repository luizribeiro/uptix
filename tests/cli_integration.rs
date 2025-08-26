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
fn test_uptix_help() {
    let output = Command::new(uptix_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Update dependencies"));
    assert!(stdout.contains("List all dependencies"));
    assert!(stdout.contains("Show detailed information"));
    assert!(stdout.contains("Initialize an empty lock file"));
}

#[test]
fn test_init_command() {
    let temp_dir = TempDir::new().unwrap();
    let lock_path = temp_dir.path().join("uptix.lock");

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute uptix init");

    assert!(output.status.success());
    assert!(lock_path.exists());

    let content = fs::read_to_string(&lock_path).unwrap();
    assert_eq!(content, "{}");
}

#[test]
fn test_init_command_existing_lock() {
    let temp_dir = TempDir::new().unwrap();
    let lock_path = temp_dir.path().join("uptix.lock");

    // Create existing lock file
    fs::write(&lock_path, r#"{"test": "value"}"#).unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute uptix init");

    assert!(output.status.success());

    // Verify lock file wasn't overwritten
    let content = fs::read_to_string(&lock_path).unwrap();
    assert_eq!(content, r#"{"test": "value"}"#);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("uptix.lock already exists"));
}

#[test]
fn test_list_no_dependencies() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute uptix list");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No uptix.lock file found"));
}

#[test]
fn test_list_with_dependencies() {
    let temp_dir = TempDir::new().unwrap();

    // Create a lock file with dependencies
    let lock_content = r#"{
        "postgres:15": "sha256:bc51cf4f1fe02cce7ed2370b20128a9b00b4eb804573a77d2a0d877aaa9c82b1",
        "redis:latest": "sha256:472f4f5ed5d4258056093ea5745bc0ada37628b667d7db4fb12c2ffea74b2703"
    }"#;
    fs::write(temp_dir.path().join("uptix.lock"), lock_content).unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .arg("list")
        .output()
        .expect("Failed to execute uptix list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dependencies in uptix.lock:"));
    assert!(stdout.contains("postgres:15"));
    assert!(stdout.contains("redis:latest"));
}

#[test]
fn test_show_dependency_not_found() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["show", "nonexistent:dep"])
        .output()
        .expect("Failed to execute uptix show");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No uptix.lock file found"));
}

#[test]
fn test_show_dependency_found() {
    let temp_dir = TempDir::new().unwrap();

    // Create a lock file with a dependency
    let lock_content = r#"{
        "postgres:15": "sha256:somehash"
    }"#;
    fs::write(temp_dir.path().join("uptix.lock"), lock_content).unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["show", "postgres:15"])
        .output()
        .expect("Failed to execute uptix show");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dependency: postgres:15"));
    assert!(stdout.contains("Locked version:"));
    assert!(stdout.contains("sha256:somehash"));
}

#[test]
fn test_update_no_args_is_default() {
    let temp_dir = TempDir::new().unwrap();

    // Create empty lock file to prevent actual updates
    fs::write(temp_dir.path().join("uptix.lock"), "{}").unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute uptix");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 0 nix files"));
}

#[test]
fn test_update_specific_dependency() {
    let temp_dir = TempDir::new().unwrap();

    // Create a nix file with dependencies
    let nix_content = r#"{
        postgres = uptix.dockerImage "postgres:15";
        redis = uptix.dockerImage "redis:latest";
    }"#;
    fs::write(temp_dir.path().join("test.nix"), nix_content).unwrap();

    // Create initial lock file
    let lock_content = r#"{
        "postgres:15": "sha256:oldhash",
        "redis:latest": "sha256:redishash"
    }"#;
    fs::write(temp_dir.path().join("uptix.lock"), lock_content).unwrap();

    let output = Command::new(uptix_binary())
        .current_dir(temp_dir.path())
        .args(&["update", "--dependency", "postgres:15"])
        .output()
        .expect("Failed to execute uptix update");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Found 1 dependencies matching 'postgres:15'"));
}

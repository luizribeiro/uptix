use uptix::deps;
use uptix::util;

use clap::{Parser, Subcommand};
use deps::{collect_file_dependencies, Dependency, LockEntry, LockFile};
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the lock file (defaults to ./uptix.lock)
    #[arg(short, long, global = true)]
    lock_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Update dependencies
    Update {
        /// Update only the specified dependency
        #[arg(short, long)]
        dependency: Option<String>,
    },
    /// List all dependencies
    List,
    /// Show detailed information about a dependency
    Show {
        /// Dependency to show details for
        dependency: String,
    },
    /// Initialize an empty lock file
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Determine root directory and lock file path
    let (root_dir, lock_path) = if let Some(lock_file) = args.lock_file {
        let lock_path_abs = lock_file.canonicalize().unwrap_or(lock_file.clone());
        let root_dir = lock_path_abs
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (root_dir, lock_path_abs)
    } else {
        (PathBuf::from("."), PathBuf::from("./uptix.lock"))
    };

    match args.command {
        Commands::Update { dependency } => {
            update_command(&root_dir, &lock_path, dependency).await
        }
        Commands::List => list_command(&lock_path),
        Commands::Show { dependency } => show_command(&lock_path, &dependency),
        Commands::Init => init_command(&lock_path),
    }
}

async fn update_command(
    root_dir: &Path,
    lock_path: &Path,
    dependency: Option<String>,
) -> Result<()> {
    update_command_in_dir(root_dir, lock_path, dependency).await
}

async fn update_command_in_dir(
    root_path: &Path,
    lock_path: &Path,
    dependency: Option<String>,
) -> Result<()> {
    // Validate lock file format first, before doing any expensive operations
    if lock_path.exists() {
        validate_lock_file(lock_path)?;
    }

    let all_files = util::discover_nix_files(root_path.to_str().unwrap());
    println!("Found {} nix files", all_files.len());

    print!("Parsing files... ");
    std::io::stdout().flush().into_diagnostic()?;
    let mut all_dependencies: Vec<Dependency> = vec![];
    for f in all_files {
        let mut deps = collect_file_dependencies(f.to_str().unwrap())?;
        all_dependencies.append(&mut deps);
    }
    println!("Done.");

    // Filter dependencies if a specific one is requested
    let dependencies_to_update = if let Some(ref dep_pattern) = dependency {
        let filtered: Vec<Dependency> = all_dependencies
            .into_iter()
            .filter(|d| d.matches(dep_pattern))
            .collect();

        if filtered.is_empty() {
            eprintln!(
                "Error: Dependency '{}' not found in the project",
                dep_pattern
            );
            return Ok(());
        }

        println!(
            "Found {} dependencies matching '{}'",
            filtered.len(),
            dep_pattern
        );
        filtered
    } else {
        println!("Found {} uptix dependencies", all_dependencies.len());
        all_dependencies
    };

    // Load existing lock file if it exists
    let mut lock_file: LockFile = if lock_path.exists() && dependency.is_some() {
        let existing_content = fs::read_to_string(&lock_path).into_diagnostic()?;
        serde_json::from_str(&existing_content).unwrap_or_else(|_| LockFile::new())
    } else {
        LockFile::new()
    };

    print!("Looking for updates... ");
    std::io::stdout().flush().into_diagnostic()?;

    for dependency in dependencies_to_update {
        let lock_entry = dependency.lock_with_metadata().await.into_diagnostic();
        if lock_entry.is_err() {
            println!("Error while updating dependency {}", dependency.key());
            println!("{:?}", lock_entry.err().unwrap());
            return Ok(());
        }
        let entry = lock_entry.unwrap();
        lock_file.insert(dependency.key().to_string(), entry);
    }
    println!("Done.");

    let mut file = fs::File::create(&lock_path).expect("Error creating uptix.lock");
    let json = serde_json::to_string_pretty(&lock_file).into_diagnostic()?;
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to uptix.lock");
    println!("Wrote uptix.lock successfully");

    return Ok(());
}

fn list_command(lock_path: &Path) -> Result<()> {
    list_command_in_dir(lock_path)
}

fn list_command_in_dir(lock_path: &Path) -> Result<()> {
    if !lock_path.exists() {
        eprintln!("Error: No uptix.lock file found. Run 'uptix update' first.");
        return Ok(());
    }

    let lock_content = fs::read_to_string(&lock_path).into_diagnostic()?;
    let lock_file: LockFile = serde_json::from_str(&lock_content).into_diagnostic()?;

    if lock_file.is_empty() {
        println!("No dependencies in uptix.lock");
        return Ok(());
    }

    println!("Dependencies in uptix.lock:");
    println!(
        "{:<35} {:<30} {:<20}",
        "DEPENDENCY", "TYPE", "LOCKED VERSION"
    );
    println!("{}", "-".repeat(85));

    for (_key, entry) in &lock_file {
        let metadata = &entry.metadata;

        let type_display = metadata.type_display(entry).into_diagnostic()?;
        let friendly = metadata.friendly_version_display(entry).into_diagnostic()?;

        println!(
            "{:<35} {:<30} {:<20}",
            metadata.name, type_display, friendly
        );
    }

    Ok(())
}

fn show_command(lock_path: &Path, dependency: &str) -> Result<()> {
    show_command_in_dir(lock_path, dependency)
}

fn show_command_in_dir(lock_path: &Path, dependency: &str) -> Result<()> {
    if !lock_path.exists() {
        eprintln!("Error: No uptix.lock file found. Run 'uptix update' first.");
        return Ok(());
    }

    let lock_content = fs::read_to_string(&lock_path).into_diagnostic()?;
    let lock_file: LockFile = serde_json::from_str(&lock_content).into_diagnostic()?;

    // Find the dependency in the lock file
    // Check for exact match first
    if let Some(entry) = lock_file.get(dependency) {
        display_dependency_details(dependency, entry)?;
        return Ok(());
    }

    // Try to find by partial match (for ergonomic patterns)
    for (key, entry) in &lock_file {
        // Check if the key matches the ergonomic patterns
        if key.contains(dependency)
            || (dependency.contains("/") && key.contains(dependency))
            || (dependency.contains(":") && key.contains(&dependency.replace(":", ":")))
        {
            display_dependency_details(key, entry)?;
            return Ok(());
        }
    }

    eprintln!("Error: Dependency '{}' not found in uptix.lock", dependency);

    Ok(())
}

fn display_dependency_details(key: &str, entry: &LockEntry) -> Result<()> {
    let metadata = &entry.metadata;

    println!("Dependency Key: {}", key);
    println!();
    println!("Name: {}", metadata.name);

    if let Some(selector) = &metadata.selected_version {
        println!("Selected Version: {}", selector);
    }

    if let Some(resolved) = &metadata.resolved_version {
        println!("Resolved Version: {}", resolved);
    } else {
        println!("Resolved Version: pending");
    }

    let friendly = metadata.friendly_version_display(entry).into_diagnostic()?;
    println!("Friendly Version: {}", friendly);

    if let Some(timestamp) = &metadata.timestamp {
        println!("Timestamp: {}", timestamp);
    }

    println!("Type: {}", metadata.dep_type);
    println!("Description: {}", metadata.description);

    println!("\nLock data:");
    println!(
        "{}",
        serde_json::to_string_pretty(&entry.lock).into_diagnostic()?
    );

    Ok(())
}

fn init_command(lock_path: &Path) -> Result<()> {
    init_command_in_dir(lock_path)
}

fn init_command_in_dir(lock_path: &Path) -> Result<()> {
    if lock_path.exists() {
        eprintln!("Error: uptix.lock already exists. Use 'uptix update' to update dependencies.");
        return Ok(());
    }

    let empty_lock: LockFile = LockFile::new();
    let json = serde_json::to_string_pretty(&empty_lock).into_diagnostic()?;
    fs::write(&lock_path, json).into_diagnostic()?;
    println!("Created empty uptix.lock file.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cli_structure() {
        // Test that the CLI structure is valid
        let cmd = Args::command();
        cmd.debug_assert();
    }

    #[test]
    fn test_parse_update_command() {
        let args = Args::parse_from(&["uptix", "update"]);
        matches!(args.command, Commands::Update { .. });
    }

    #[test]
    fn test_parse_update_with_dependency() {
        let args = Args::parse_from(&["uptix", "update", "--dependency", "postgres:15"]);
        match args.command {
            Commands::Update { dependency } => {
                assert_eq!(dependency, Some("postgres:15".to_string()));
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_parse_list_command() {
        let args = Args::parse_from(&["uptix", "list"]);
        matches!(args.command, Commands::List);
    }

    #[test]
    fn test_parse_show_command() {
        let args = Args::parse_from(&["uptix", "show", "postgres:15"]);
        match args.command {
            Commands::Show { dependency } => {
                assert_eq!(dependency, "postgres:15");
            }
            _ => panic!("Expected Show command"),
        }
    }

    #[test]
    fn test_parse_init_command() {
        let args = Args::parse_from(&["uptix", "init"]);
        matches!(args.command, Commands::Init);
    }

    #[test]
    fn test_init_creates_empty_lock() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let lock_path = temp_path.join("uptix.lock");

        // Run init command in temp directory
        let result = init_command_in_dir(&lock_path);
        assert!(result.is_ok());

        // Verify lock file was created
        assert!(lock_path.exists());
        let content = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(content, "{}");
    }

    #[test]
    fn test_init_fails_if_lock_exists() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let lock_path = temp_path.join("uptix.lock");

        // Create existing lock file
        fs::write(&lock_path, "{}").unwrap();

        // Run init command - should not error but print message
        let result = init_command_in_dir(&lock_path);
        assert!(result.is_ok());

        // Verify lock file wasn't overwritten
        let content = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(content, "{}");
    }

    #[test]
    fn test_list_with_no_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("uptix.lock");

        // No lock file exists, should succeed with error message
        let result = list_command_in_dir(&lock_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_dependency_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("uptix.lock");

        // No lock file exists, should succeed with error message
        let result = show_command_in_dir(&lock_path, "nonexistent:dep");
        assert!(result.is_ok()); // Command succeeds but prints error message
    }

    #[test]
    fn test_parse_short_flags() {
        // Test short flag -d for dependency
        let args = Args::parse_from(&["uptix", "update", "-d", "postgres:15"]);
        match args.command {
            Commands::Update { dependency } => {
                assert_eq!(dependency, Some("postgres:15".to_string()));
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_parse_invalid_command() {
        // Use try_parse_from to test parsing without panicking
        let result = Args::try_parse_from(&["uptix", "invalid-command"]);
        assert!(result.is_err());

        if let Err(e) = result {
            let err_str = e.to_string();
            assert!(err_str.contains("unrecognized subcommand"));
        }
    }

    #[test]
    fn test_show_with_lock_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let lock_path = temp_path.join("uptix.lock");

        // Create a lock file with a postgres dependency
        let lock_content = r#"{
            "postgres:15": {
                "metadata": {
                    "name": "postgres",
                    "selected_version": "15",
                    "resolved_version": "sha256:bc51cf4f1fe0",
                    "dep_type": "docker",
                    "description": "Docker image postgres:15"
                },
                "lock": "sha256:bc51cf4f1fe02cce7ed2370b20128a9b00b4eb804573a77d2a0d877aaa9c82b1"
            }
        }"#;
        fs::write(&lock_path, lock_content).unwrap();

        // Run show command
        let result = show_command_in_dir(&lock_path, "postgres:15");
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_with_lock_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let lock_path = temp_path.join("uptix.lock");

        // Create a lock file with multiple dependencies
        let lock_content = r#"{
            "postgres:15": {
                "metadata": {
                    "name": "postgres",
                    "selected_version": "15",
                    "resolved_version": "sha256:bc51cf4f1fe0",
                    "dep_type": "docker",
                    "description": "Docker image postgres:15"
                },
                "lock": "sha256:bc51cf4f1fe02cce7ed2370b20128a9b00b4eb804573a77d2a0d877aaa9c82b1"
            },
            "redis:latest": {
                "metadata": {
                    "name": "redis",
                    "selected_version": "latest",
                    "resolved_version": "sha256:472f4f5ed5d4",
                    "dep_type": "docker",
                    "description": "Docker image redis:latest"
                },
                "lock": "sha256:472f4f5ed5d4258056093ea5745bc0ada37628b667d7db4fb12c2ffea74b2703"
            }
        }"#;
        fs::write(&lock_path, lock_content).unwrap();

        // Run list command
        let result = list_command_in_dir(&lock_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_preserves_existing_entries() {
        // This is tested via integration tests since it requires actual async operations
        // See test_single_dep.sh for the full integration test
    }
}

fn validate_lock_file(lock_path: &Path) -> Result<()> {
    let existing_content = fs::read_to_string(&lock_path).into_diagnostic()?;

    // Try to parse as uptix lock file
    match serde_json::from_str::<LockFile>(&existing_content) {
        Ok(lock) => {
            // Validate it's actually an uptix lock by checking if entries have metadata
            if !lock.is_empty() {
                let first_entry = lock.values().next().unwrap();
                if first_entry.metadata.dep_type.is_empty() {
                    eprintln!("❌ Error: '{}' doesn't appear to be an uptix.lock file.", lock_path.display());
                    eprintln!("   Expected JSON with 'metadata' and 'lock' fields.");
                    eprintln!("   Did you mean to specify 'uptix.lock' instead?");
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Err(e) => {
            if existing_content.trim().starts_with('{') {
                // It's JSON but not uptix format
                eprintln!("❌ Error: '{}' doesn't appear to be an uptix.lock file.", lock_path.display());
                eprintln!("   Parse error: {}", e);
                eprintln!("   Did you mean to specify 'uptix.lock' instead?");
            } else {
                eprintln!("❌ Error: '{}' is not a valid JSON file.", lock_path.display());
            }
            std::process::exit(1);
        }
    }
}

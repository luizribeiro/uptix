use uptix::deps;
use uptix::util;

use deps::collect_file_dependencies;
use deps::Dependency;
use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Update dependencies (default if no command specified)
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
    
    // Default to update command if none specified
    let command = args.command.unwrap_or(Commands::Update { dependency: None });
    
    match command {
        Commands::Update { dependency } => update_command(dependency).await,
        Commands::List => list_command(),
        Commands::Show { dependency } => show_command(&dependency),
        Commands::Init => init_command(),
    }
}

async fn update_command(dependency: Option<String>) -> Result<()> {
    update_command_in_dir(".", dependency).await
}

async fn update_command_in_dir(root_path: &str, dependency: Option<String>) -> Result<()> {
    let all_files = util::discover_nix_files(root_path);
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
            eprintln!("Error: Dependency '{}' not found in the project", dep_pattern);
            return Ok(());
        }

        println!("Found {} dependencies matching '{}'", filtered.len(), dep_pattern);
        filtered
    } else {
        println!("Found {} uptix dependencies", all_dependencies.len());
        all_dependencies
    };

    let lock_path = Path::new(root_path).join("uptix.lock");
    
    // Load existing lock file if it exists
    let mut lock_file: BTreeMap<String, Value> =
        if lock_path.exists() && dependency.is_some() {
            let existing_content = fs::read_to_string(&lock_path).into_diagnostic()?;
            serde_json::from_str(&existing_content).unwrap_or_else(|_| BTreeMap::new())
        } else {
            BTreeMap::new()
        };

    print!("Looking for updates... ");
    std::io::stdout().flush().into_diagnostic()?;

    for dependency in dependencies_to_update {
        let lock = dependency.lock().await.into_diagnostic();
        if lock.is_err() {
            println!("Error while updating dependency {}", dependency.key());
            println!("{:?}", lock.err().unwrap());
            return Ok(());
        }
        let lock_value = lock.unwrap();
        let json_value = serde_json::to_value(lock_value).into_diagnostic()?;
        lock_file.insert(dependency.key().to_string(), json_value);
    }
    println!("Done.");

    let mut file = fs::File::create(&lock_path).expect("Error creating uptix.lock");
    let json = serde_json::to_string_pretty(&lock_file).into_diagnostic()?;
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to uptix.lock");
    println!("Wrote uptix.lock successfully");

    return Ok(());
}

fn list_command() -> Result<()> {
    list_command_in_dir(".")
}

fn list_command_in_dir(root_path: &str) -> Result<()> {
    let all_files = util::discover_nix_files(root_path);
    let mut all_dependencies: Vec<Dependency> = vec![];
    
    for f in all_files {
        let mut deps = collect_file_dependencies(f.to_str().unwrap())?;
        all_dependencies.append(&mut deps);
    }
    
    if all_dependencies.is_empty() {
        println!("No uptix dependencies found in the project.");
        return Ok(());
    }
    
    println!("Dependencies found in project:");
    for dep in all_dependencies {
        println!("  {}", dep.key());
    }
    
    Ok(())
}

fn show_command(dependency: &str) -> Result<()> {
    show_command_in_dir(".", dependency)
}

fn show_command_in_dir(root_path: &str, dependency: &str) -> Result<()> {
    let all_files = util::discover_nix_files(root_path);
    let mut found_dep: Option<Dependency> = None;
    
    for f in all_files {
        let deps = collect_file_dependencies(f.to_str().unwrap())?;
        for dep in deps {
            if dep.matches(dependency) {
                found_dep = Some(dep);
                break;
            }
        }
        if found_dep.is_some() {
            break;
        }
    }
    
    match found_dep {
        Some(dep) => {
            println!("Dependency: {}", dep.key());
            
            let lock_path = Path::new(root_path).join("uptix.lock");
            // Load lock file to show current locked version
            if lock_path.exists() {
                let lock_content = fs::read_to_string(&lock_path).into_diagnostic()?;
                let lock_file: BTreeMap<String, Value> = serde_json::from_str(&lock_content).into_diagnostic()?;
                
                if let Some(locked_value) = lock_file.get(&dep.key()) {
                    println!("\nCurrent locked version:");
                    println!("{}", serde_json::to_string_pretty(locked_value).into_diagnostic()?);
                } else {
                    println!("\nNot currently locked.");
                }
            } else {
                println!("\nNo lock file found.");
            }
        }
        None => {
            eprintln!("Error: Dependency '{}' not found in the project", dependency);
        }
    }
    
    Ok(())
}

fn init_command() -> Result<()> {
    init_command_in_dir(".")
}

fn init_command_in_dir(root_path: &str) -> Result<()> {
    let lock_path = Path::new(root_path).join("uptix.lock");
    if lock_path.exists() {
        eprintln!("Error: uptix.lock already exists. Use 'uptix update' to update dependencies.");
        return Ok(());
    }
    
    let empty_lock: BTreeMap<String, Value> = BTreeMap::new();
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
        matches!(args.command, Some(Commands::Update { .. }));
    }

    #[test]
    fn test_parse_update_with_dependency() {
        let args = Args::parse_from(&["uptix", "update", "--dependency", "postgres:15"]);
        match args.command {
            Some(Commands::Update { dependency }) => {
                assert_eq!(dependency, Some("postgres:15".to_string()));
            }
            _ => panic!("Expected Update command"),
        }
    }

    #[test]
    fn test_parse_list_command() {
        let args = Args::parse_from(&["uptix", "list"]);
        matches!(args.command, Some(Commands::List));
    }

    #[test]
    fn test_parse_show_command() {
        let args = Args::parse_from(&["uptix", "show", "postgres:15"]);
        match args.command {
            Some(Commands::Show { dependency }) => {
                assert_eq!(dependency, "postgres:15");
            }
            _ => panic!("Expected Show command"),
        }
    }

    #[test]
    fn test_parse_init_command() {
        let args = Args::parse_from(&["uptix", "init"]);
        matches!(args.command, Some(Commands::Init));
    }

    #[test]
    fn test_default_to_update() {
        let args = Args::parse_from(&["uptix"]);
        assert!(args.command.is_none());
        // In main(), this gets converted to Commands::Update { dependency: None }
    }

    #[test]
    fn test_init_creates_empty_lock() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Run init command in temp directory
        let result = init_command_in_dir(temp_path.to_str().unwrap());
        assert!(result.is_ok());
        
        // Verify lock file was created
        let lock_path = temp_path.join("uptix.lock");
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
        let result = init_command_in_dir(temp_path.to_str().unwrap());
        assert!(result.is_ok());
        
        // Verify lock file wasn't overwritten
        let content = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(content, "{}");
    }

    #[test]
    fn test_list_with_no_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        
        // Run list command
        let result = list_command_in_dir(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_dependency_not_found() {
        let temp_dir = TempDir::new().unwrap();
        
        // Run show command
        let result = show_command_in_dir(temp_dir.path().to_str().unwrap(), "nonexistent:dep");
        assert!(result.is_ok()); // Command succeeds but prints error message
    }

    #[test]
    fn test_parse_short_flags() {
        // Test short flag -d for dependency
        let args = Args::parse_from(&["uptix", "update", "-d", "postgres:15"]);
        match args.command {
            Some(Commands::Update { dependency }) => {
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
    fn test_show_with_nix_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a simple nix file
        let nix_content = r#"{
            postgres = uptix.dockerImage "postgres:15";
        }"#;
        fs::write(temp_path.join("test.nix"), nix_content).unwrap();
        
        // Run show command
        let result = show_command_in_dir(temp_path.to_str().unwrap(), "postgres:15");
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_with_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create multiple nix files
        let nix_content1 = r#"{
            postgres = uptix.dockerImage "postgres:15";
        }"#;
        let nix_content2 = r#"{
            redis = uptix.dockerImage "redis:latest";
        }"#;
        fs::write(temp_path.join("db.nix"), nix_content1).unwrap();
        fs::write(temp_path.join("cache.nix"), nix_content2).unwrap();
        
        // Run list command
        let result = list_command_in_dir(temp_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_preserves_existing_entries() {
        // This is tested via integration tests since it requires actual async operations
        // See test_single_dep.sh for the full integration test
    }
}
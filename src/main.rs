mod deps;
mod error;
mod util;

#[macro_use]
extern crate lazy_static;

use crate::deps::collect_file_dependencies;
use crate::deps::Dependency;
use clap::Parser;
use miette::{IntoDiagnostic, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Update only the specified dependency (by key)
    #[arg(short, long)]
    dependency: Option<String>,
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let all_files = util::discover_nix_files(".");
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
    let dependencies_to_update = if let Some(ref dep_pattern) = args.dependency {
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

    // Load existing lock file if it exists
    let mut lock_file: BTreeMap<String, Value> =
        if Path::new("uptix.lock").exists() && args.dependency.is_some() {
            let existing_content = fs::read_to_string("uptix.lock").into_diagnostic()?;
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

    let mut file = fs::File::create("uptix.lock").expect("Error creating uptix.lock");
    let json = serde_json::to_string_pretty(&lock_file).into_diagnostic()?;
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to uptix.lock");
    println!("Wrote uptix.lock successfully");

    return Ok(());
}

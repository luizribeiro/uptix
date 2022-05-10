mod deps;
mod error;
mod util;

#[macro_use]
extern crate lazy_static;

use crate::deps::collect_file_dependencies;
use crate::deps::Dependency;
use miette::{IntoDiagnostic, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
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
    println!("Found {} uptix dependencies", all_dependencies.len());

    print!("Looking for updates... ");
    std::io::stdout().flush().into_diagnostic()?;
    let mut lock_file = BTreeMap::new();
    for dependency in all_dependencies {
        let lock = dependency.lock().await.into_diagnostic()?;
        lock_file.insert(dependency.key().to_string(), lock);
    }
    println!("Done.");

    let mut file = fs::File::create("uptix.lock").expect("Error creating uptix.lock");
    let json = serde_json::to_string_pretty(&lock_file).into_diagnostic()?;
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to uptix.lock");
    println!("Wrote uptix.lock successfully");

    return Ok(());
}

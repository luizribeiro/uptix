mod deps;
mod error;
mod util;

#[macro_use]
extern crate lazy_static;

use crate::deps::collect_file_dependencies;
use crate::error::Error;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let all_files = util::discover_nix_files(".");
    println!("Found {} nix files", all_files.len());

    print!("Parsing files... ");
    std::io::stdout().flush()?;
    let all_dependencies: Vec<_> = all_files
        .iter()
        .map(|f| collect_file_dependencies(f.to_str().unwrap()).unwrap())
        .flatten()
        .collect();
    println!("Done.");
    println!("Found {} uptix dependencies", all_dependencies.len());

    print!("Looking for updates... ");
    std::io::stdout().flush()?;
    let mut lock_file = BTreeMap::new();
    for dependency in all_dependencies {
        let lock = dependency.lock().await?;
        lock_file.insert(dependency.key().to_string(), lock);
    }
    println!("Done.");

    let mut file = fs::File::create("uptix.lock").expect("Error creating uptix.lock");
    let json = serde_json::to_string_pretty(&lock_file)?;
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to uptix.lock");
    println!("Wrote uptix.lock successfully");

    return Ok(());
}

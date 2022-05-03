mod deps;
mod docker;
mod util;

#[macro_use]
extern crate lazy_static;

use crate::deps::collect_file_dependencies;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let all_files = util::discover_nix_files(".");
    println!("Found {} nix files", all_files.len());

    print!("Parsing files... ");
    std::io::stdout().flush().unwrap();
    let all_dependencies: Vec<_> = all_files
        .iter()
        .map(|f| collect_file_dependencies(f.to_str().unwrap()))
        .flatten()
        .collect();
    println!("Done.");
    println!("Found {} docknix dependencies", all_dependencies.len());

    print!("Looking for updates... ");
    std::io::stdout().flush().unwrap();
    let mut lock_file = BTreeMap::new();
    for dependency in all_dependencies {
        let lock = dependency.lock().await?;
        lock_file.insert(
            dependency.key().to_string(),
            lock,
        );
    }
    println!("Done.");

    let mut file = fs::File::create("docknix.lock")
        .expect("Error creating docknix.lock");
    let json = serde_json::to_string_pretty(&lock_file).unwrap();
    file.write_all(json.as_bytes())
        .expect("Error writing JSON to docknix.lock");
    println!("Wrote docknix.lock successfully");

    return Ok(());
}

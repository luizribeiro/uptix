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
async fn main() {
    let all_files = util::discover_nix_files();
    println!("Found {} nix files", all_files.len());

    print!("Parsing files... ");
    std::io::stdout().flush().unwrap();
    let all_dependencies: Vec<_> = all_files
        .iter()
        .map(|f| collect_file_dependencies(f.to_str().unwrap()))
        .flatten()
        .collect();
    println!("Done.");
    println!("Found {} docker image references", all_dependencies.len());

    print!("Looking for updates... ");
    std::io::stdout().flush().unwrap();
    let mut lock_file = BTreeMap::new();
    for dependency in all_dependencies {
        let lock = dependency.lock().await.unwrap();
        lock_file.insert(
            dependency.key().to_string(),
            lock,
        );
    }
    println!("Done.");

    let mut file = fs::File::create("docknix.lock").unwrap();
    file.write_all(serde_json::to_string_pretty(&lock_file).unwrap().as_bytes()).unwrap();
    println!("Wrote docknix.lock successfully");
}

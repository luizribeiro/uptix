mod backend;
mod docker;
mod util;

#[macro_use]
extern crate lazy_static;

use crate::backend::Backend;
use rnix::{SyntaxKind, SyntaxNode};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

fn extract_docker_images(file_path: &str) -> Vec<Box<dyn Backend>> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return collect_dependencies(ast.node());
}

fn collect_dependencies(node: SyntaxNode) -> Vec<Box<dyn Backend>> {
    if node.kind() != SyntaxKind::NODE_APPLY {
        let mut dependencies = Vec::new();
        for child in node.children() {
            dependencies.append(&mut collect_dependencies(child));
        }
        return dependencies;
    }

    let mut children = node.children();
    let select_node = match children.next() {
        Some(n) => match n.kind() {
            SyntaxKind::NODE_SELECT => n,
            _ => return vec![],
        },
        _ => return vec![],
    };

    let func = select_node.text().to_string();
    if !func.starts_with("docknix.") {
        return vec![];
    }

    let value_node = match children.next() {
        Some(n) => n,
        None => return vec![],
    };

    let dependency = match <dyn Backend>::new(&func, &value_node) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    return vec![dependency];
}

#[tokio::main]
async fn main() {
    let all_files = util::discover_nix_files();
    println!("Found {} nix files", all_files.len());

    print!("Parsing files... ");
    std::io::stdout().flush().unwrap();
    let mut all_docker_images = vec![];
    for file in all_files {
        all_docker_images.append(&mut extract_docker_images(file.to_str().unwrap()));
    }
    println!("Done.");
    println!("Found {} docker image references", all_docker_images.len());

    print!("Looking for updates... ");
    std::io::stdout().flush().unwrap();
    let mut lock_file = BTreeMap::new();
    for dep in all_docker_images {
        let lock = dep.get_lock().await.unwrap();
        lock_file.insert(
            dep.get_lock_key().to_string(),
            lock,
        );
    }
    println!("Done.");

    let mut file = fs::File::create("docknix.lock").unwrap();
    file.write_all(serde_json::to_string_pretty(&lock_file).unwrap().as_bytes()).unwrap();
    println!("Wrote docknix.lock successfully");
}

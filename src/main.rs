mod backend;
mod docker;
mod util;

use crate::backend::Backend;
use crate::docker::Docker;
use rnix::{SyntaxKind, SyntaxNode};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

fn extract_docker_images(file_path: &str) -> Vec<Box<dyn Backend>> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return visit(ast.node());
}

fn visit(node: SyntaxNode) -> Vec<Box<dyn Backend>> {
    // lol this is wonky AF
    if node.kind() != SyntaxKind::NODE_APPLY {
        let mut images = Vec::new();
        for child in node.children() {
            images.append(&mut visit(child));
        }
        return images;
    }

    let mut children = node.children();
    let select = children.next();
    if select.is_none() || select.as_ref().unwrap().kind() != SyntaxKind::NODE_SELECT {
        return Vec::new();
    }

    if select.as_ref().unwrap().text() != "docknix.image" {
        return Vec::new();
    }

    let string = children.next();
    if string.is_none() {
        return vec![];
    }

    let dep = Docker::new(&string.unwrap()).unwrap();
    return vec![Box::new(dep)];
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

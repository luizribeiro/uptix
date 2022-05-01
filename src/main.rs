use dkregistry::v2::Client;
use glob::glob;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::fs;
use rnix::{SyntaxKind, SyntaxNode};

fn discover_nix_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in glob("**/*.nix").unwrap() {
        if let Ok(path) = entry {
            files.push(path);
        }
    }
    return files;
}

fn visit(node: SyntaxNode) -> Vec<String> {
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
    let s = string.unwrap();
    if s.kind() != SyntaxKind::NODE_STRING {
        return vec![];
    }

    let mut x = s.text().to_string();
    x.pop();
    x.remove(0);
    return vec![x];
}

fn extract_docker_images(file_path: &str) -> Vec<String> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return visit(ast.node());
}

fn get_image_components(raw_image: &str) -> (&str, &str, &str) {
    let re = Regex::new(r"(?:([a-z0-9.-]+)/)?([a-z0-9-]+/[a-z0-9-]+):?([a-z0-9.-]+)?").unwrap();
    let caps = re.captures(raw_image).unwrap();

    let registry = caps.get(1).map_or("registry-1.docker.io", |m| m.as_str());
    let image = caps.get(2).map(|m| m.as_str()).unwrap();
    let tag = caps.get(3).map_or("latest", |m| m.as_str());

    return (registry, image, tag);
}

async fn get_digest<'a>(
    registry: &'a str,
    image: &'a str,
    tag: &'a str,
) -> Option<String> {
    let client = Client::configure().registry(registry).build().unwrap();
    let login_scope = format!("repository:{}:pull", image);
    let dclient = client.authenticate(&[&login_scope]).await.unwrap();
    return dclient.get_manifestref(image, tag).await.unwrap();
}

#[tokio::main]
async fn main() {
    let mut all_docker_images = vec![];
    for file in discover_nix_files() {
        all_docker_images.append(&mut extract_docker_images(file.to_str().unwrap()));
    }

    let mut lock = BTreeMap::new();
    for name in all_docker_images {
        let (registry, image, tag) = get_image_components(name.as_str());

        let digest = get_digest(registry, image, tag).await.unwrap();
        lock.insert(
            name.to_string(),
            format!("{}@{}", name, digest.to_string()),
        );
    }

    let output = serde_json::to_string_pretty(&lock).unwrap();
    println!("{}", output);
}

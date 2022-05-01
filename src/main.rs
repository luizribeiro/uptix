use dkregistry::v2::Client;
use regex::Regex;
use rnix::{SyntaxKind, SyntaxNode};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use walkdir::{DirEntry, WalkDir};

fn is_not_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| entry.depth() == 0 || !s.starts_with("."))
         .unwrap_or(false)
}

fn discover_nix_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkDir::new(".").into_iter();
    for entry in walker.filter_entry(|e| is_not_hidden(e)) {
        let e = entry.unwrap();
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("nix") {
            continue;
        }
        files.push(PathBuf::from(path));
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
    let all_files = discover_nix_files();
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
    let mut lock = BTreeMap::new();
    for name in all_docker_images {
        let (registry, image, tag) = get_image_components(name.as_str());

        let digest = get_digest(registry, image, tag).await.unwrap();
        lock.insert(
            name.to_string(),
            format!("{}@{}", name, digest.to_string()),
        );
    }
    println!("Done.");

    let mut file = fs::File::create("docknix.lock").unwrap();
    file.write_all(serde_json::to_string_pretty(&lock).unwrap().as_bytes()).unwrap();
    println!("Wrote docknix.lock successfully");
}

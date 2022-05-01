use dkregistry::v2::Client;
use glob::glob;
use regex::Regex;
use rnix::types::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{env, fs};

fn discover_nix_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in glob("**/*.nix").unwrap() {
        if let Ok(path) = entry {
            files.push(path);
        }
    }
    return files;
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
    let mut iter = env::args().skip(1).peekable();
    let files = discover_nix_files();
    println!("{:?}", files);
    if iter.peek().is_none() {
        eprintln!("Usage: docknix <file>");
        return;
    }
    let file = iter.next().unwrap();
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error: {}", err);
            return;
        }
    };
    let ast = rnix::parse(&content);
    let set = ast.root().inner().and_then(AttrSet::cast).unwrap();
    let mut lock = BTreeMap::new();
    for entry in set.entries() {
        let key = entry.key().unwrap();
        let ident = key.path().last().and_then(Ident::cast);
        let name = ident.as_ref().map_or("error", Ident::as_str);

        let value = entry.value().unwrap();
        let token = value.to_string();
        let raw_image = &token[1..token.len() - 1];

        let (registry, image, tag) = get_image_components(raw_image);

        let digest = get_digest(registry, image, tag).await.unwrap();
        lock.insert(
            name.to_string(),
            format!("{}@{}", raw_image, digest.to_string()),
        );
    }
    let output = serde_json::to_string_pretty(&lock).unwrap();
    println!("{}", output);
}

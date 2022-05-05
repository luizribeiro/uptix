use rnix::{SyntaxKind, SyntaxNode};
use serde_json::{Map, Value};
use std::path::PathBuf;
use walkdir::{DirEntry, WalkDir};

fn is_not_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with("."))
        .unwrap_or(false)
}

pub fn discover_nix_files(root_path: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkDir::new(root_path).into_iter();
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

pub fn user_agent() -> String {
    return format!("uptix/{}", env!("CARGO_PKG_VERSION"));
}

fn extract_key_value(node: &SyntaxNode) -> (String, String) {
    let key = node.first_child().unwrap();
    let mut value = key.next_sibling().unwrap().text().to_string();
    value.pop();
    value.remove(0);
    return (key.text().to_string(), value);
}

pub fn from_attr_set<T>(node: &SyntaxNode) -> Result<T, &'static str>
where
    T: serde::de::DeserializeOwned,
{
    if node.kind() != SyntaxKind::NODE_ATTR_SET {
        return Err("Unexpected node");
    }

    let mut attrs: Map<String, serde_json::Value> = Map::new();
    for child in node.children() {
        if child.kind() != SyntaxKind::NODE_KEY_VALUE {
            return Err("Unexpected node");
        }
        let (key, value) = extract_key_value(&child);
        attrs.insert(key, serde_json::Value::String(value));
    }

    let json = Value::Object(attrs).to_string();
    return Ok(serde_json::from_str::<T>(&json).unwrap());
}

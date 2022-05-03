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

use crate::error::Error;
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

pub struct ParsingContext {
    file_path: String,
    file_contents: String,
}

impl ParsingContext {
    pub fn new(file_path: &str, file_contents: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
            file_contents: file_contents.to_string(),
        }
    }

    pub fn src(&self) -> miette::NamedSource {
        miette::NamedSource::new(self.file_path.clone(), self.file_contents.clone())
    }
}

fn value_from_nix(node: &SyntaxNode) -> Result<Value, Error> {
    if node.kind() == SyntaxKind::NODE_STRING {
        let mut w = node.text().to_string();
        w.pop();
        w.remove(0);
        return Ok(serde_json::Value::String(w));
    }

    if node.kind() == SyntaxKind::NODE_LITERAL {
        let token = node.first_token().unwrap();
        return match token.kind() {
            SyntaxKind::TOKEN_INTEGER => {
                let v = token.text().parse::<i32>().unwrap();
                Ok(serde_json::Value::from(v))
            }
            SyntaxKind::TOKEN_FLOAT => {
                let v = token.text().parse::<f32>().unwrap();
                Ok(serde_json::Value::from(v))
            }
            _ => Err(Error::NixParsingError(format!(
                "Unexpected token kind {:#?}",
                token.kind()
            ))),
        };
    }

    if node.kind() == SyntaxKind::NODE_IDENT {
        return match node.text().to_string().as_str() {
            "true" => Ok(serde_json::Value::Bool(true)),
            "false" => Ok(serde_json::Value::Bool(false)),
            identifier => Err(Error::NixParsingError(format!(
                "Unexpected identifier {}",
                identifier,
            ))),
        };
    }

    if node.kind() != SyntaxKind::NODE_ATTR_SET {
        return Err(Error::NixParsingError(format!(
            "Expected attr set, found {:#?}",
            node.kind()
        )));
    }

    let mut attrs: Map<String, serde_json::Value> = Map::new();
    for child in node.children() {
        if child.kind() != SyntaxKind::NODE_KEY_VALUE {
            return Err(Error::NixParsingError(format!(
                "Expected key/value pair, got {:#?}",
                child.kind()
            )));
        }
        let key = child.first_child().unwrap();
        let value = key.next_sibling().unwrap();
        attrs.insert(key.text().to_string(), value_from_nix(&value)?);
    }

    return Ok(Value::Object(attrs));
}

pub fn from_attr_set<T>(node: &SyntaxNode) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let value = value_from_nix(node)?;
    let json = value.to_string();
    return Ok(serde_json::from_str::<T>(&json).unwrap());
}

#[cfg(test)]
mod tests {
    use super::from_attr_set;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    pub struct A {
        a: String,
        b: B,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    pub struct B {
        b: String,
        c: i32,
        d: f32,
        e: Option<i32>,
        f: Option<i32>,
    }

    #[test]
    fn it_deserializes_attr_sets() {
        let ast = rnix::parse(
            r#"{
                a = "foo";
                b = {
                    b = "bar";
                    c = 42;
                    d = 3.1415;
                    f = 7;
                };
            }"#,
        );
        let attrset = ast.node().first_child().unwrap();
        assert_eq!(
            from_attr_set::<A>(&attrset).unwrap(),
            A {
                a: "foo".to_string(),
                b: B {
                    b: "bar".to_string(),
                    c: 42,
                    d: 3.1415,
                    e: None,
                    f: Some(7),
                }
            },
        );
    }
}

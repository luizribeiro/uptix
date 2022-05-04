use crate::deps::Lockable;
use async_trait::async_trait;
use erased_serde::Serialize;
use rnix::{SyntaxKind, SyntaxNode};
use std::collections::HashMap;

#[derive(PartialEq, Clone, Debug)]
pub struct GitHub {
    owner: String,
    repo: String,
    branch: String,
}

impl GitHub {
    pub fn new(node: &SyntaxNode) -> Result<GitHub, &'static str> {
        if node.kind() != SyntaxKind::NODE_ATTR_SET {
            return Err("Unexpected node");
        }

        let mut args: HashMap<String, String> = HashMap::new();
        for child in node.children() {
            if child.kind() != SyntaxKind::NODE_KEY_VALUE {
                return Err("Unexpected node");
            }
            let (key, value) = extract_key_value(&child);
            args.insert(key, value);
        }

        return Ok(GitHub {
            owner: args.get("owner").unwrap().to_string(),
            repo: args.get("repo").unwrap().to_string(),
            branch: args.get("branch").unwrap().to_string(),
        });
    }
}

fn extract_key_value(node: &SyntaxNode) -> (String, String) {
    let key = node.first_child().unwrap();
    let mut value = key.next_sibling().unwrap().text().to_string();
    value.pop();
    value.remove(0);
    return (key.text().to_string(), value);
}

#[async_trait]
impl Lockable for GitHub {
    fn key(&self) -> String {
        return String::from("");
    }

    async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str> {
        return Ok(Box::new("TODO"));
    }
}

#[cfg(test)]
mod tests {
    use crate::deps::collect_ast_dependencies;
    use super::GitHub;

    #[test]
    fn it_parses() {
        let ast = rnix::parse(
            r#"{
                uptix = fetchFromGitHub (uptix.github {
                    owner = "luizribeiro";
                    repo = "uptix";
                    branch = "main";
                });
            }"#,
        );
        let dependencies: Vec<_> = collect_ast_dependencies(ast.node())
            .iter()
            .map(|d| d.as_git_hub().unwrap().clone())
            .collect();
        let expected_dependencies = vec![
            GitHub {
                owner: "luizribeiro".to_string(),
                repo: "uptix".to_string(),
                branch: "main".to_string(),
            },
        ];
        assert_eq!(dependencies, expected_dependencies);
    }
}

mod docker;

use crate::deps::docker::Docker;
use async_trait::async_trait;
use erased_serde::Serialize;
use rnix::{SyntaxKind, SyntaxNode};
use std::fs;

pub enum Dependency {
    Docker(Docker),
}

#[async_trait]
pub trait Lockable {
    fn key(&self) -> &str;
    async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str>;
}

impl Dependency {
    pub fn new(func: &str, node: &SyntaxNode) -> Result<Dependency, &'static str> {
        let dep = match func {
            "uptix.dockerImage" => Dependency::Docker(Docker::new(&node)?),
            _ => return Err("Unknown uptix function"),
        };
        return Ok(dep);
    }

    pub fn key(&self) -> &str {
        match self {
            Dependency::Docker(d) => &d.key(),
        }
    }

    pub async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str> {
        match self {
            Dependency::Docker(d) => d.lock().await,
        }
    }
}

pub fn collect_file_dependencies(file_path: &str) -> Vec<Dependency> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return collect_ast_dependencies(ast.node());
}

fn collect_ast_dependencies(node: SyntaxNode) -> Vec<Dependency> {
    if node.kind() != SyntaxKind::NODE_APPLY {
        return node
            .children()
            .map(|c| collect_ast_dependencies(c))
            .flatten()
            .collect();
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
    if !func.starts_with("uptix.") {
        return vec![];
    }

    let value_node = match children.next() {
        Some(n) => n,
        None => return vec![],
    };

    let dependency = match <Dependency>::new(&func, &value_node) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    return vec![dependency];
}

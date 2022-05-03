mod docker;

use async_trait::async_trait;
use crate::deps::docker::Docker;
use erased_serde::Serialize;
use rnix::{SyntaxKind, SyntaxNode};
use std::fs;

#[async_trait]
pub trait Dependency {
    fn key(&self) -> &str;
    async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str>;
}

impl dyn Dependency {
    pub fn new(
        func: &str,
        node: &SyntaxNode,
    ) -> Result<Box<dyn Dependency>, &'static str> {
        let dep = match func {
            "uptix.dockerImage" => Docker::new(&node)?,
            _ => return Err("Unknown uptix function"),
        };
        return Ok(Box::new(dep));
    }
}

pub fn collect_file_dependencies(file_path: &str) -> Vec<Box<dyn Dependency>> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return collect_ast_dependencies(ast.node());
}

fn collect_ast_dependencies(node: SyntaxNode) -> Vec<Box<dyn Dependency>> {
    if node.kind() != SyntaxKind::NODE_APPLY {
        return node.children()
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

    let dependency = match <dyn Dependency>::new(&func, &value_node) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    return vec![dependency];
}

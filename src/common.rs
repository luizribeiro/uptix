use async_trait::async_trait;
use crate::docker::Docker;
use rnix::SyntaxNode;

#[async_trait]
pub trait Dependency {
    fn key(&self) -> &str;
    async fn lock(&self) -> Result<String, &'static str>;
}

impl dyn Dependency {
    pub fn new(
        func: &str,
        node: &SyntaxNode,
    ) -> Result<Box<dyn Dependency>, &'static str> {
        let dep = match func {
            "docknix.image" => Docker::new(&node)?,
            _ => return Err("Unknown docknix function"),
        };
        return Ok(Box::new(dep));
    }
}

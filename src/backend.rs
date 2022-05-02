use async_trait::async_trait;
use crate::docker::Docker;
use rnix::SyntaxNode;

#[async_trait]
pub trait Backend {
    fn get_lock_key(&self) -> &str;
    async fn get_lock(&self) -> Result<String, &'static str>;
}

impl dyn Backend {
    pub fn new(
        func: &str,
        node: &SyntaxNode,
    ) -> Result<Box<dyn Backend>, &'static str> {
        let dep = match func {
            "docknix.image" => Docker::new(&node)?,
            _ => return Err("Unknown docknix function"),
        };
        return Ok(Box::new(dep));
    }
}

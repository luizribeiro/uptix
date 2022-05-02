use async_trait::async_trait;
use crate::backend::Backend;
use dkregistry::v2::Client;
use regex::Regex;
use rnix::{SyntaxKind, SyntaxNode};

pub struct Docker {
    name: String,
    registry: String,
    image: String,
    tag: String,
}

impl Docker {
    pub fn new(node: &SyntaxNode) -> Result<Docker, &'static str> {
        if node.kind() != SyntaxKind::NODE_STRING {
            return Err("Unexpected node");
        }

        let mut name = node.text().to_string();
        name.pop();
        name.remove(0);

        let (registry, image, tag) = get_image_components(name.as_str())?;
        return Ok(Docker { name, registry, image, tag });
    }
}

#[async_trait]
impl Backend for Docker {
    fn get_lock_key(&self) -> &str {
        return &self.name;
    }

    async fn get_lock(&self) -> Option<String> {
        let client = Client::configure()
            .registry(self.registry.as_str())
            .build()
            .unwrap();
        let login_scope = format!("repository:{}:pull", self.image);
        let dclient = client.authenticate(&[&login_scope]).await.unwrap();
        let digest = dclient.get_manifestref(
            self.image.as_str(),
            self.tag.as_str(),
        ).await.unwrap().unwrap();
        return Some(format!("{}@{}", self.name, digest));
    }
}

lazy_static! {
    static ref RE: Regex =
        Regex::new(r"(?:([a-z0-9.-]+)/)?([a-z0-9-]+/[a-z0-9-]+):?([a-z0-9.-]+)?")
        .unwrap();
}

fn get_image_components(
    raw_image: &str,
) -> Result<(String, String, String), &'static str> {
    let caps = match RE.captures(raw_image) {
        Some(c) => c,
        _ => return Err("Malformatted Docker image"),
    };
    let registry = caps.get(1).map_or("registry-1.docker.io", |m| m.as_str());
    let image = match caps.get(2).map(|m| m.as_str()) {
        Some(i) => i,
        _ => return Err("Invalid Docker image name"),
    };
    let tag = caps.get(3).map_or("latest", |m| m.as_str());

    return Ok((registry.to_string(), image.to_string(), tag.to_string()));
}

use async_trait::async_trait;
use crate::backend::Backend;
use dkregistry::v2::Client;
use dkregistry::errors::Error as RegistryError;
use regex::Regex;
use rnix::{SyntaxKind, SyntaxNode};

pub struct Docker {
    name: String,
    registry: String,
    image: String,
    tag: String,
}

lazy_static! {
    static ref RE: Regex =
        Regex::new(r#"((?:([a-z0-9.-]+)/)?([a-z0-9-]+/[a-z0-9-]+):?([a-z0-9.-]+)?)"#)
        .unwrap();
}

impl Docker {
    pub fn new(node: &SyntaxNode) -> Result<Docker, &'static str> {
        if node.kind() != SyntaxKind::NODE_STRING {
            return Err("Unexpected node");
        }

        let text = node.text().to_string();
        return Docker::from(text.as_str());
    }

    fn from(text: &str) -> Result<Docker, &'static str> {
        let caps = match RE.captures(text) {
            Some(c) => c,
            _ => return Err("Malformatted Docker image"),
        };
        let name = match caps.get(1).map(|m| m.as_str()) {
            Some(i) => i.to_string(),
            _ => return Err("Invalid Docker image name"),
        };
        let registry = caps.get(2)
            .map_or("registry-1.docker.io", |m| m.as_str())
            .to_string();
        let image = match caps.get(3).map(|m| m.as_str()) {
            Some(i) => i.to_string(),
            _ => return Err("Invalid Docker image"),
        };
        let tag = caps.get(4)
            .map_or("latest", |m| m.as_str())
            .to_string();

        return Ok(Docker { name, registry, image, tag });
    }

    async fn get_latest_digest(&self) -> Result<Option<String>, RegistryError> {
        let client = Client::configure()
            .registry(self.registry.as_str())
            .build()?;
        let login_scope = format!("repository:{}:pull", self.image);
        let dclient = client.authenticate(&[&login_scope]).await?;
        let digest = dclient.get_manifestref(
            self.image.as_str(),
            self.tag.as_str(),
        ).await?;
        return Ok(digest);
    }
}

#[async_trait]
impl Backend for Docker {
    fn get_lock_key(&self) -> &str {
        return &self.name;
    }

    async fn get_lock(&self) -> Result<String, &'static str> {
        return match self.get_latest_digest().await {
            Ok(Some(digest)) => Ok(format!("{}@{}", self.name, digest)),
            Ok(None) => Err("Could not find digest for image on registry"),
            Err(_err) => Err("Error while fetching digest from registry"),
        };
    }
}

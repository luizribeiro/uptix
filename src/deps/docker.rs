use async_trait::async_trait;
use crate::deps::Lockable;
use dkregistry::errors::Error as RegistryError;
use dkregistry::v2::Client;
use erased_serde::Serialize;
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
        let caps = RE.captures(text).expect("Malformatted Docker image");
        let name = caps.get(1).map(|m| m.as_str())
            .expect("Invalid Docker image name")
            .to_string();
        let registry = caps.get(2)
            .map_or("registry-1.docker.io", |m| m.as_str())
            .to_string();
        let image = caps.get(3).map(|m| m.as_str())
            .expect("Invalid Docker image")
            .to_string();
        let tag = caps.get(4)
            .map_or("latest", |m| m.as_str())
            .to_string();

        return Ok(Docker { name, registry, image, tag });
    }

    async fn latest_digest(&self) -> Result<Option<String>, RegistryError> {
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
impl Lockable for Docker {
    fn key(&self) -> &str {
        return &self.name;
    }

    async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str> {
        return match self.latest_digest().await {
            Ok(Some(digest)) => Ok(Box::new(digest)),
            Ok(None) => Err("Could not find digest for image on registry"),
            Err(_err) => Err("Error while fetching digest from registry"),
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::deps::Dependency::Docker;
    use crate::deps::Lockable;
    use crate::deps::collect_ast_dependencies;

    #[test]
    fn it_parses() {
        let ast = rnix::parse("{
            hass = uptix.dockerImage \"homeassistant/home-assistant:stable\";
            customRepo = uptix.dockerImage \"foo.io/baz/bar:latest\";
        }");
        let dependencies = collect_ast_dependencies(ast.node());
        assert_eq!(dependencies.len(), 2);
        let Docker(dependency) = dependencies.get(0).unwrap();
        assert_eq!(dependency.name, "homeassistant/home-assistant:stable");
        assert_eq!(dependency.registry, "registry-1.docker.io");
        assert_eq!(dependency.image, "homeassistant/home-assistant");
        assert_eq!(dependency.tag, "stable");
        let Docker(dependency) = dependencies.get(1).unwrap();
        assert_eq!(dependency.name, "foo.io/baz/bar:latest");
        assert_eq!(dependency.registry, "foo.io");
        assert_eq!(dependency.image, "baz/bar");
        assert_eq!(dependency.tag, "latest");
    }

    #[tokio::test]
    async fn it_locks() {
        let dependency = super::Docker {
            name: "homeassistant/home-assistant:stable".to_string(),
            registry: "registry-1.docker.io".to_string(),
            image: "homeassistant/home-assistant".to_string(),
            tag: "stable".to_string(),
        };
        // TODO: mock network and improve test
        let lock = dependency.lock().await.unwrap();
        let lock_value = serde_json::to_value(lock).unwrap();
        assert!(lock_value.as_str().unwrap().starts_with(r#"sha256"#));
    }
}

use crate::deps::{assert_kind, Lockable};
use crate::error::Error;
use crate::util::ParsingContext;
use async_trait::async_trait;
use dkregistry::mediatypes::MediaTypes;
use dkregistry::v2::Client;
use erased_serde::Serialize;
use regex::Regex;
use rnix::{SyntaxKind, SyntaxNode};

#[derive(PartialEq, Clone, Debug)]
pub struct Docker {
    name: String,
    registry: String,
    image: String,
    tag: String,
    use_https: bool,
}

const DEFAULT_REGISTRY: &str = "registry-1.docker.io";
const DEFAULT_TAG: &str = "latest";

lazy_static! {
    static ref RE: Regex =
        Regex::new(r#"((?:([a-z0-9.-]+)/)?([a-z0-9-]+/[a-z0-9-]+):?([a-z0-9.-]+)?)"#).unwrap();
}

impl Docker {
    pub fn new(context: &ParsingContext, node: &SyntaxNode) -> Result<Docker, Error> {
        let string_node = assert_kind(
            context,
            "uptix.dockerImage",
            node,
            SyntaxKind::NODE_STRING,
            r#"here are some examples of allowed parameters:
 - homeassistant/home-assistant:stable
 - grafana/grafana
 - custom.registry.io/foo/bar:tag"#,
        )?;
        let text = string_node.text().to_string();
        return Docker::from(text.as_str());
    }

    fn from(text: &str) -> Result<Docker, Error> {
        let caps = RE.captures(text).expect("Malformatted Docker image");
        let name = caps
            .get(1)
            .map(|m| m.as_str())
            .expect("Invalid Docker image name")
            .to_string();
        let registry = caps
            .get(2)
            .map_or(DEFAULT_REGISTRY, |m| m.as_str())
            .to_string();
        let image = caps
            .get(3)
            .map(|m| m.as_str())
            .expect("Invalid Docker image")
            .to_string();
        let tag = caps.get(4).map_or(DEFAULT_TAG, |m| m.as_str()).to_string();

        return Ok(Docker {
            name,
            registry,
            image,
            tag,
            use_https: true,
        });
    }

    async fn latest_digest(&self) -> Result<Option<String>, Error> {
        let login_scope = format!("repository:{}:pull", self.image);
        let scopes = vec![login_scope.as_str()];
        let dclient = Client::configure()
            .registry(self.registry.as_str())
            .insecure_registry(!self.use_https)
            .accepted_types(Some(vec![
                (MediaTypes::ManifestV2S2, Some(0.5)),
                (MediaTypes::ManifestV2S1Signed, Some(0.4)),
                (MediaTypes::ManifestList, Some(0.5)),
                (MediaTypes::OCIImageIndexV1, Some(0.5)),
            ]))
            .build()?
            .authenticate(scopes.as_slice())
            .await?;
        let digest = dclient
            .get_manifestref(self.image.as_str(), self.tag.as_str())
            .await?;
        return Ok(digest);
    }
}

#[async_trait]
impl Lockable for Docker {
    fn key(&self) -> String {
        return self.name.to_string();
    }

    async fn lock(&self) -> Result<Box<dyn Serialize>, Error> {
        return match self.latest_digest().await? {
            Some(digest) => Ok(Box::new(digest)),
            None => Err(Error::StringError(format!(
                "Could not find digest for image {} on registry",
                self.name,
            ))),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Docker;
    use crate::deps::test_util;
    use crate::deps::Lockable;

    #[test]
    fn it_parses() {
        let dependencies: Vec<_> = test_util::deps(
            r#"{
            hass = uptix.dockerImage "homeassistant/home-assistant:stable";
            customRepo = uptix.dockerImage "foo.io/baz/bar";
        }"#,
        )
        .unwrap()
        .iter()
        .map(|d| d.as_docker().unwrap().clone())
        .collect();
        let expected_dependencies = vec![
            Docker {
                name: "homeassistant/home-assistant:stable".to_string(),
                registry: "registry-1.docker.io".to_string(),
                image: "homeassistant/home-assistant".to_string(),
                tag: "stable".to_string(),
                use_https: true,
            },
            Docker {
                name: "foo.io/baz/bar".to_string(),
                registry: "foo.io".to_string(),
                image: "baz/bar".to_string(),
                tag: "latest".to_string(),
                use_https: true,
            },
        ];
        assert_eq!(dependencies, expected_dependencies);
    }

    #[tokio::test]
    async fn it_locks() {
        let registry = mockito::server_address().to_string();
        let _auth_mock = mockito::mock("GET", "/v2/")
            .with_status(200)
            .with_header(
                "WWW-Authenticate",
                format!(r#"Bearer realm="http://{}/token""#, registry).as_str(),
            )
            .with_body("{}")
            .create();
        let _token_mock = mockito::mock("GET", "/token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"token": "hunter2"}"#)
            .create();
        let _manifest_mock =
            mockito::mock("HEAD", "/v2/homeassistant/home-assistant/manifests/stable")
                .with_status(200)
                .with_header("docker-content-digest", "sha256:foobar")
                .create();

        let dependency = Docker {
            name: "homeassistant/home-assistant:stable".to_string(),
            registry,
            image: "homeassistant/home-assistant".to_string(),
            tag: "stable".to_string(),
            use_https: false,
        };
        let lock = dependency.lock().await.unwrap();
        let lock_value = serde_json::to_value(lock).unwrap();

        assert_eq!(lock_value.as_str().unwrap(), "sha256:foobar");
        mockito::reset();
    }

    #[test]
    fn it_provides_helpful_errors() {
        let result = test_util::deps("{ hass = uptix.dockerImage 42; }");
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::UnexpectedArgument {
                function,
                src: _,
                argument_pos,
                expected_type,
                help: _,
            }) => {
                assert_eq!(function, "uptix.dockerImage");
                assert_eq!(expected_type, "NODE_STRING");
                assert_eq!(argument_pos, (27, 2).into());
            }
            _ => assert!(false),
        }
    }
}

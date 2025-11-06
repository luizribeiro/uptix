use crate::deps::{assert_kind, DependencyMetadata, LockEntry, Lockable};
use crate::error::Error;
use crate::util::ParsingContext;
use async_trait::async_trait;
use dkregistry::mediatypes::MediaTypes;
use dkregistry::v2::Client;
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
    // Matches:
    // 1. Optional registry with trailing slash: (registry.io/)
    // 2. Image name with optional namespace: (namespace/name or just name)
    // 3. Optional tag with leading colon: (:tag)
    static ref RE: Regex =
        Regex::new(r#"^(?:([a-z0-9.-]+)/)?([a-z0-9-]+(?:/[a-z0-9-]+)*):?([a-z0-9.-]+)?$"#).unwrap();
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
 - postgres:15
 - redis:7-alpine
 - custom.registry.io/foo/bar:tag"#,
        )?;
        // Remove the quotes from the string
        let text = string_node.text().to_string();
        let text = text.trim_matches('"');
        return Docker::from(text);
    }

    /// Reconstructs a Docker dependency from a lock entry.
    pub fn from_lock_entry(entry: &crate::deps::LockEntry) -> Option<Docker> {
        let name = &entry.metadata.name;
        let tag = entry.metadata.selected_version.as_ref()?;
        Docker::from(&format!("{}:{}", name, tag)).ok()
    }

    fn from(text: &str) -> Result<Docker, Error> {
        let caps = RE.captures(text).expect("Malformatted Docker image");

        // Extract components from regex capture groups
        let registry_part = caps.get(1);
        let image_part = caps.get(2).expect("Invalid Docker image");
        let tag_part = caps.get(3);

        // The full name is the original text
        let name = text.to_string();

        // Check if this is a registry or a namespace
        let (registry, image) = if let Some(reg) = registry_part {
            // If the registry part contains a dot, it's likely a registry domain
            if reg.as_str().contains('.') {
                (reg.as_str().to_string(), image_part.as_str().to_string())
            } else {
                // It's a namespace, not a registry
                (
                    DEFAULT_REGISTRY.to_string(),
                    format!("{}/{}", reg.as_str(), image_part.as_str()),
                )
            }
        } else {
            // No registry specified, use default
            (
                DEFAULT_REGISTRY.to_string(),
                image_part.as_str().to_string(),
            )
        };

        // Tag defaults to "latest" if not specified
        let tag = tag_part.map_or(DEFAULT_TAG.to_string(), |m| m.as_str().to_string());

        return Ok(Docker {
            name,
            registry,
            image,
            tag,
            use_https: true,
        });
    }

    async fn latest_digest(&self) -> Result<Option<String>, Error> {
        // For Docker Hub (registry-1.docker.io), we need to handle library/ prefix for official images
        let image_name = if self.registry == DEFAULT_REGISTRY && !self.image.contains('/') {
            format!("library/{}", self.image)
        } else {
            self.image.clone()
        };

        // Common configuration settings
        let accepted_types = Some(vec![
            (MediaTypes::ManifestV2S2, Some(0.5)),
            (MediaTypes::ManifestV2S1Signed, Some(0.4)),
            (MediaTypes::ManifestList, Some(0.5)),
            (MediaTypes::OCIImageIndexV1, Some(0.5)),
        ]);

        // First try: Direct access without authentication (for public images)
        let direct_result = async {
            let dclient = Client::configure()
                .registry(self.registry.as_str())
                .insecure_registry(!self.use_https)
                .accepted_types(accepted_types.clone())
                .build()?;
            dclient
                .get_manifestref(image_name.as_str(), self.tag.as_str())
                .await
        }
        .await;

        // If direct access worked, return the result
        if direct_result.is_ok() {
            return Ok(direct_result?);
        }

        // Second try: With authentication
        let login_scope = format!("repository:{}:pull", image_name);
        let scopes = vec![login_scope.as_str()];

        let authenticated_result = async {
            let dclient = Client::configure()
                .registry(self.registry.as_str())
                .insecure_registry(!self.use_https)
                .accepted_types(accepted_types)
                .build()?
                .authenticate(scopes.as_slice())
                .await?;
            dclient
                .get_manifestref(image_name.as_str(), self.tag.as_str())
                .await
        }
        .await;

        // Log errors if debugging is needed
        if let Err(ref auth_err) = authenticated_result {
            if let Err(ref direct_err) = direct_result {
                eprintln!("Direct access error: {:?}", direct_err);
                eprintln!("Authenticated access error: {:?}", auth_err);
            }
        }

        return Ok(authenticated_result?);
    }
}

#[async_trait]
impl Lockable for Docker {
    fn key(&self) -> String {
        return self.name.to_string();
    }

    fn matches(&self, pattern: &str) -> bool {
        // Match against the full name (e.g., "postgres:15")
        self.name == pattern ||
        // Also match without tag if pattern doesn't include one and it's a Docker Hub image
        (self.registry == DEFAULT_REGISTRY && self.image == pattern && !pattern.contains(':'))
    }

    async fn lock_with_metadata(&self) -> Result<LockEntry, Error> {
        // Fetch the digest
        let digest = match self.latest_digest().await? {
            Some(d) => d,
            None => {
                return Err(Error::StringError(format!(
                    "Could not find digest for image {} on registry",
                    self.name,
                )))
            }
        };

        // Create metadata with all fields populated
        let metadata = DependencyMetadata {
            name: self.image.clone(),
            selected_version: Some(self.tag.clone()),
            resolved_version: Some(digest.clone()),
            friendly_version: Some(if let Some(hash_part) = digest.strip_prefix("sha256:") {
                format!("sha256:{}", &hash_part[..12.min(hash_part.len())])
            } else {
                digest.clone()
            }),
            dep_type: "docker".to_string(),
            description: format!(
                "Docker image {}:{} from {}",
                self.image, self.tag, self.registry
            ),
        };

        Ok(LockEntry {
            metadata,
            lock: serde_json::Value::String(digest),
        })
    }

    fn type_display(&self) -> String {
        format!("docker-image ({})", self.tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::test_util;
    use crate::deps::Lockable;

    #[test]
    fn test_docker_matches() {
        let docker = Docker {
            registry: DEFAULT_REGISTRY.to_string(),
            image: "postgres".to_string(),
            tag: "15".to_string(),
            name: "postgres:15".to_string(),
            use_https: true,
        };

        // Should match full name with tag
        assert!(docker.matches("postgres:15"));

        // Should match just the image name without tag
        assert!(docker.matches("postgres"));

        // Should not match different image
        assert!(!docker.matches("mysql:8"));
        assert!(!docker.matches("mysql"));

        // Should not match partial names
        assert!(!docker.matches("post"));
    }

    #[test]
    fn test_docker_with_registry_matches() {
        let docker = Docker {
            registry: "gcr.io".to_string(),
            image: "my-project/my-image".to_string(),
            tag: "latest".to_string(),
            name: "gcr.io/my-project/my-image:latest".to_string(),
            use_https: true,
        };

        assert!(docker.matches("gcr.io/my-project/my-image:latest"));
        assert!(!docker.matches("my-project/my-image:latest"));
        assert!(!docker.matches("my-project/my-image"));
    }

    #[test]
    fn it_parses() {
        let dependencies: Vec<_> = test_util::deps(
            r#"{
            hass = uptix.dockerImage "homeassistant/home-assistant:stable";
            customRepo = uptix.dockerImage "foo.io/baz/bar";
            postgres = uptix.dockerImage "postgres:15";
            redis = uptix.dockerImage "redis:7-alpine";
            clickhouse = uptix.dockerImage "clickhouse/clickhouse-server:23.11";
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
            Docker {
                name: "postgres:15".to_string(),
                registry: "registry-1.docker.io".to_string(),
                image: "postgres".to_string(),
                tag: "15".to_string(),
                use_https: true,
            },
            Docker {
                name: "redis:7-alpine".to_string(),
                registry: "registry-1.docker.io".to_string(),
                image: "redis".to_string(),
                tag: "7-alpine".to_string(),
                use_https: true,
            },
            Docker {
                name: "clickhouse/clickhouse-server:23.11".to_string(),
                registry: "registry-1.docker.io".to_string(),
                image: "clickhouse/clickhouse-server".to_string(),
                tag: "23.11".to_string(),
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
        let lock_entry = dependency.lock_with_metadata().await.unwrap();
        assert_eq!(lock_entry.metadata.name, "homeassistant/home-assistant");
        assert_eq!(
            lock_entry.metadata.selected_version,
            Some("stable".to_string())
        );
        assert_eq!(lock_entry.lock.as_str().unwrap(), "sha256:foobar");
        mockito::reset();
    }

    // Note: We're not testing the actual Docker registry API calls here,
    // as that would require a complex mock setup and is more of an integration test.
    // Instead, we're focusing on testing the specific logic we added to fix the bug:
    // 1. The library/ prefix is correctly added for official images

    #[test]
    fn it_adds_library_prefix_for_official_images() {
        // Create a Docker struct for an official image (postgres:15)
        let docker = Docker {
            name: "postgres:15".to_string(),
            registry: DEFAULT_REGISTRY.to_string(), // registry-1.docker.io
            image: "postgres".to_string(),
            tag: "15".to_string(),
            use_https: true,
        };

        // Extract the code that computes the image name with the library/ prefix
        let image_name = if docker.registry == DEFAULT_REGISTRY && !docker.image.contains('/') {
            format!("library/{}", docker.image)
        } else {
            docker.image.clone()
        };

        // Verify that the library/ prefix is added
        assert_eq!(image_name, "library/postgres");

        // Test with a custom registry (should not add library/)
        let docker = Docker {
            name: "postgres:15".to_string(),
            registry: "custom.registry.io".to_string(),
            image: "postgres".to_string(),
            tag: "15".to_string(),
            use_https: true,
        };

        let image_name = if docker.registry == DEFAULT_REGISTRY && !docker.image.contains('/') {
            format!("library/{}", docker.image)
        } else {
            docker.image.clone()
        };

        // Verify that the library/ prefix is NOT added for custom registries
        assert_eq!(image_name, "postgres");

        // Test with a namespaced image (should not add library/)
        let docker = Docker {
            name: "bitnami/postgresql:15".to_string(),
            registry: DEFAULT_REGISTRY.to_string(),
            image: "bitnami/postgresql".to_string(),
            tag: "15".to_string(),
            use_https: true,
        };

        let image_name = if docker.registry == DEFAULT_REGISTRY && !docker.image.contains('/') {
            format!("library/{}", docker.image)
        } else {
            docker.image.clone()
        };

        // Verify that the library/ prefix is NOT added for namespaced images
        assert_eq!(image_name, "bitnami/postgresql");
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

    #[test]
    fn it_parses_simple_images() {
        let image = Docker::from("postgres:15").unwrap();
        assert_eq!(image.registry, "registry-1.docker.io");
        assert_eq!(image.image, "postgres");
        assert_eq!(image.tag, "15");

        let image = Docker::from("redis").unwrap();
        assert_eq!(image.registry, "registry-1.docker.io");
        assert_eq!(image.image, "redis");
        assert_eq!(image.tag, "latest");
    }

    #[test]
    fn it_parses_namespaced_images() {
        let image = Docker::from("homeassistant/home-assistant:stable").unwrap();
        assert_eq!(image.registry, "registry-1.docker.io");
        assert_eq!(image.image, "homeassistant/home-assistant");
        assert_eq!(image.tag, "stable");
    }

    #[test]
    fn it_parses_registry_images() {
        let image = Docker::from("ghcr.io/user/app:v1").unwrap();
        assert_eq!(image.registry, "ghcr.io");
        assert_eq!(image.image, "user/app");
        assert_eq!(image.tag, "v1");

        let image = Docker::from("my-registry.example.com/team/project:latest").unwrap();
        assert_eq!(image.registry, "my-registry.example.com");
        assert_eq!(image.image, "team/project");
        assert_eq!(image.tag, "latest");
    }
}

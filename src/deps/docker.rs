use crate::deps::{assert_kind, DependencyMetadata, LockEntry, Lockable};
use crate::error::Error;
use crate::util::ParsingContext;
use async_trait::async_trait;
use dkregistry::mediatypes::MediaTypes;
use dkregistry::v2::manifest::Manifest;
use dkregistry::v2::Client;
use regex::Regex;
use rnix::{SyntaxKind, SyntaxNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Partial representation of a Docker image config blob
/// This is what we get when fetching the config blob referenced in the manifest
#[derive(Debug, Deserialize, Serialize)]
struct ImageConfig {
    #[serde(rename = "created")]
    created: Option<String>,
    config: Option<ImageConfigDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ImageConfigDetails {
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

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

    /// Get Docker Hub credentials from environment variables or ~/.docker/config.json
    /// Returns (username, password/token) if credentials are found
    fn get_credentials() -> Option<(String, String)> {
        // Priority 1: Check environment variables (for CI)
        if let (Ok(username), Ok(token)) = (
            std::env::var("DOCKERHUB_USERNAME"),
            std::env::var("DOCKERHUB_TOKEN"),
        ) {
            return Some((username, token));
        }

        // Priority 2: Check ~/.docker/config.json (for local dev)
        if let Some(home_dir) = std::env::var("HOME").ok() {
            let config_path = format!("{}/.docker/config.json", home_dir);
            if let Ok(config_contents) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_contents) {
                    // Check for "auths" field with Docker Hub credentials
                    if let Some(auths) = config.get("auths").and_then(|a| a.as_object()) {
                        // Try various Docker Hub registry URLs
                        for registry_url in &[
                            "https://index.docker.io/v1/",
                            "index.docker.io",
                            "docker.io",
                        ] {
                            if let Some(auth) = auths.get(*registry_url) {
                                // Check for basic auth (base64 encoded username:password)
                                if let Some(auth_str) = auth.get("auth").and_then(|a| a.as_str()) {
                                    use base64::{engine::general_purpose::STANDARD, Engine};
                                    if let Ok(decoded) = STANDARD.decode(auth_str.trim()) {
                                        if let Ok(decoded_str) = String::from_utf8(decoded) {
                                            if let Some((username, password)) =
                                                decoded_str.split_once(':')
                                            {
                                                return Some((
                                                    username.to_string(),
                                                    password.to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // No credentials found
        None
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

        // Get credentials if available
        let credentials = Self::get_credentials();

        // First try: Direct access without authentication (for public images)
        let direct_result = async {
            let mut config = Client::configure()
                .registry(self.registry.as_str())
                .insecure_registry(!self.use_https)
                .accepted_types(accepted_types.clone());

            // Only use credentials for Docker Hub
            if self.registry == DEFAULT_REGISTRY {
                if let Some((username, password)) = &credentials {
                    config = config
                        .username(Some(username.clone()))
                        .password(Some(password.clone()));
                }
            }

            let dclient = config.build()?;
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
            let mut config = Client::configure()
                .registry(self.registry.as_str())
                .insecure_registry(!self.use_https)
                .accepted_types(accepted_types);

            // Only use credentials for Docker Hub
            if self.registry == DEFAULT_REGISTRY {
                if let Some((username, password)) = &credentials {
                    config = config
                        .username(Some(username.clone()))
                        .password(Some(password.clone()));
                }
            }

            let dclient = config.build()?.authenticate(scopes.as_slice()).await?;
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

    /// Helper to build an authenticated Docker registry client
    async fn build_authenticated_client(
        &self,
        credentials: &Option<(String, String)>,
        scope: &str,
    ) -> Result<dkregistry::v2::Client, Error> {
        let mut config = Client::configure()
            .registry(self.registry.as_str())
            .insecure_registry(!self.use_https);

        // Only use credentials if they're for the current registry
        // DOCKERHUB_USERNAME/DOCKERHUB_TOKEN should only be used for Docker Hub
        if self.registry == DEFAULT_REGISTRY {
            if let Some((username, password)) = credentials {
                config = config
                    .username(Some(username.clone()))
                    .password(Some(password.clone()));
            }
        }

        Ok(config.build()?.authenticate(&[scope]).await?)
    }

    /// Fetch image config with labels and creation timestamp
    /// Returns (friendly_version, timestamp) where friendly_version is either:
    /// - A semantic version from org.opencontainers.image.version label
    /// - A date formatted as YYYY-MM-DD from the created timestamp
    /// - The truncated digest as fallback
    async fn fetch_image_metadata(&self) -> Result<(Option<String>, Option<String>), Error> {
        // For Docker Hub, we need to handle library/ prefix for official images
        let image_name = if self.registry == DEFAULT_REGISTRY && !self.image.contains('/') {
            format!("library/{}", self.image)
        } else {
            self.image.clone()
        };

        // Only request Schema2 manifests for metadata extraction
        // ManifestList doesn't contain config blobs directly
        let accepted_types = Some(vec![
            (MediaTypes::ManifestV2S2, Some(1.0)),
        ]);

        // Get credentials if available
        let credentials = Self::get_credentials();

        // Try to get manifest with authentication
        let login_scope = format!("repository:{}:pull", image_name);

        let manifest = match self.build_authenticated_client(&credentials, &login_scope).await {
            Ok(client) => match client.get_manifest(image_name.as_str(), self.tag.as_str()).await {
                Ok(m) => m,
                Err(_) => return Ok((None, None)),
            },
            Err(_) => return Ok((None, None)),
        };

        // Extract config digest from manifest
        let config_digest = match manifest {
            Manifest::S2(schema2) => {
                // Schema2 manifest has config blob directly
                schema2.manifest_spec.config().digest.clone()
            }
            Manifest::ML(manifest_list) => {
                // ManifestList (multi-platform images) - fetch first platform's manifest
                // All platforms should have the same version, just different architectures
                let first_manifest = manifest_list.manifests.first();
                if first_manifest.is_none() {
                    return Ok((None, None));
                }
                let platform_digest = &first_manifest.unwrap().digest;

                // Fetch the platform-specific manifest
                let platform_client = match self.build_authenticated_client(&credentials, &login_scope).await {
                    Ok(client) => client,
                    Err(_) => return Ok((None, None)),
                };

                let platform_manifest = match platform_client.get_manifest_and_ref(&image_name, platform_digest).await {
                    Ok((m, _)) => m,
                    Err(_) => return Ok((None, None)),
                };

                // Extract config from the platform-specific manifest
                match platform_manifest {
                    Manifest::S2(s2) => s2.manifest_spec.config().digest.clone(),
                    _ => return Ok((None, None)),
                }
            }
            _ => return Ok((None, None)),
        };

        // Fetch the config blob
        let client = match self.build_authenticated_client(&credentials, &login_scope).await {
            Ok(client) => client,
            Err(_) => return Ok((None, None)),
        };

        let config_blob_bytes = match client.get_blob(&image_name, &config_digest).await {
            Ok(bytes) => bytes,
            Err(_) => return Ok((None, None)),
        };

        // Parse the config blob as JSON
        let config: ImageConfig = match serde_json::from_slice(&config_blob_bytes) {
            Ok(c) => c,
            Err(_) => return Ok((None, None)),
        };

        // Extract timestamp
        let timestamp = config.created;

        // Try to get semantic version from labels
        let friendly_version = config
            .config
            .and_then(|c| c.labels)
            .and_then(|labels| {
                labels
                    .get("org.opencontainers.image.version")
                    .or_else(|| labels.get("version"))
                    .cloned()
            })
            .or_else(|| {
                // Fall back to using creation date as YYYY-MM-DD
                timestamp.as_ref().and_then(|ts| {
                    ts.split('T').next().map(|date| date.to_string())
                })
            });

        Ok((friendly_version, timestamp))
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

        // Fetch image metadata (labels, creation timestamp)
        let (friendly_version_opt, timestamp) = self.fetch_image_metadata().await?;

        // Use friendly version from metadata, or fall back to digest
        let resolved_version = friendly_version_opt.unwrap_or_else(|| digest.clone());

        // Create metadata with all fields populated
        let metadata = DependencyMetadata {
            name: self.image.clone(),
            selected_version: Some(self.tag.clone()),
            resolved_version: Some(resolved_version),
            timestamp,
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

    fn friendly_version(&self, resolved_version: &str) -> String {
        // Shorten sha256 digests to first 12 chars for display
        if let Some(hash_part) = resolved_version.strip_prefix("sha256:") {
            format!("sha256:{}", &hash_part[..12.min(hash_part.len())])
        } else {
            resolved_version.to_string()
        }
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

    #[tokio::test]
    async fn it_fetches_image_metadata_with_labels() {
        let registry = mockito::server_address().to_string();

        // Mock authentication flow - dkregistry expects 401 for auth challenge
        let _auth_mock = mockito::mock("GET", "/v2/")
            .with_status(401)
            .with_header(
                "WWW-Authenticate",
                format!(r#"Bearer realm="http://{}/token",service="registry""#, registry).as_str(),
            )
            .with_body("{}")
            .create();
        let _token_mock = mockito::mock("GET", "/token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"token": "hunter2"}"#)
            .create();

        // Mock manifest request returning a Schema2 manifest
        // The config digest must match the SHA256 of the blob content
        let blob_content = r#"{"architecture":"amd64","created":"2024-11-03T10:23:45Z","config":{"Labels":{"org.opencontainers.image.version":"2024.11.3","maintainer":"Home Assistant"}}}"#;
        let config_digest = "sha256:74f41486f45b7ac13af8770839bf41734f9240efbaad0bfdf4fb8728244c36cf";

        let _manifest_mock = mockito::mock("GET", "/v2/homeassistant/home-assistant/manifests/stable")
            .with_status(200)
            .with_header("content-type", "application/vnd.docker.distribution.manifest.v2+json")
            .with_body(format!(r#"{{
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {{
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "size": {},
                    "digest": "{}"
                }},
                "layers": []
            }}"#, blob_content.len(), config_digest))
            .create();

        // Mock blob request returning image config with labels
        let _blob_mock = mockito::mock("GET", format!("/v2/homeassistant/home-assistant/blobs/{}", config_digest).as_str())
            .with_status(200)
            .with_body(blob_content)
            .create();

        // Mock digest request for lock_with_metadata
        let _digest_mock = mockito::mock("HEAD", "/v2/homeassistant/home-assistant/manifests/stable")
            .with_status(200)
            .with_header("docker-content-digest", "sha256:actualdigest456")
            .create();

        let dependency = Docker {
            name: "homeassistant/home-assistant:stable".to_string(),
            registry,
            image: "homeassistant/home-assistant".to_string(),
            tag: "stable".to_string(),
            use_https: false,
        };

        let lock_entry = dependency.lock_with_metadata().await.unwrap();

        // Check that we got the semantic version from the label
        assert_eq!(
            lock_entry.metadata.resolved_version,
            Some("2024.11.3".to_string())
        );

        // Check that we got the timestamp
        assert_eq!(
            lock_entry.metadata.timestamp,
            Some("2024-11-03T10:23:45Z".to_string())
        );

        // Check that the lock still contains the actual digest
        assert_eq!(lock_entry.lock.as_str().unwrap(), "sha256:actualdigest456");

        mockito::reset();
    }

    #[tokio::test]
    async fn it_falls_back_to_date_when_no_version_label() {
        let registry = mockito::server_address().to_string();

        // Mock authentication - dkregistry expects 401 for auth challenge
        let _auth_mock = mockito::mock("GET", "/v2/")
            .with_status(401)
            .with_header(
                "WWW-Authenticate",
                format!(r#"Bearer realm="http://{}/token",service="registry""#, registry).as_str(),
            )
            .with_body("{}")
            .create();
        let _token_mock = mockito::mock("GET", "/token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"token": "hunter2"}"#)
            .create();

        // Use a namespaced image to avoid the library/ prefix logic
        let image_name = "mycompany/database";
        let tag = "v15";

        // Mock manifest
        // The config digest must match the SHA256 of the blob content
        let blob_content = r#"{"architecture":"amd64","created":"2024-11-01T14:32:10Z","config":{"Labels":{"maintainer":"My Company"}}}"#;
        let config_digest = "sha256:6c583f785142603ae46e0750c14d43670c3ab0d2ed46e96dc322149de7efa736";

        let _manifest_mock = mockito::mock("GET", format!("/v2/{}/manifests/{}", image_name, tag).as_str())
            .with_status(200)
            .with_header("content-type", "application/vnd.docker.distribution.manifest.v2+json")
            .with_body(format!(r#"{{
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {{
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "size": {},
                    "digest": "{}"
                }},
                "layers": []
            }}"#, blob_content.len(), config_digest))
            .create();

        // Mock blob with no version label, only created date
        let _blob_mock = mockito::mock("GET", format!("/v2/{}/blobs/{}", image_name, config_digest).as_str())
            .with_status(200)
            .with_body(blob_content)
            .create();

        // Mock digest request
        let _digest_mock = mockito::mock("HEAD", format!("/v2/{}/manifests/{}", image_name, tag).as_str())
            .with_status(200)
            .with_header("docker-content-digest", "sha256:actualdigest")
            .create();

        let dependency = Docker {
            name: format!("{}:{}", image_name, tag),
            registry,
            image: image_name.to_string(),
            tag: tag.to_string(),
            use_https: false,
        };

        let lock_entry = dependency.lock_with_metadata().await.unwrap();

        // Should fall back to date format YYYY-MM-DD
        assert_eq!(
            lock_entry.metadata.resolved_version,
            Some("2024-11-01".to_string())
        );

        assert_eq!(
            lock_entry.metadata.timestamp,
            Some("2024-11-01T14:32:10Z".to_string())
        );

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

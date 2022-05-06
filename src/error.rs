#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("registry error")]
    RegistryError(#[from] dkregistry::errors::Error),
    #[error("HTTP request error")]
    RequestError(#[from] reqwest::Error),
    #[error("URL construction error")]
    URLConstructionError(#[from] url::ParseError),
    #[error("JSON serialization error")]
    JSONParsingError(#[from] serde_json::Error),
    #[error("I/O error")]
    IOError(#[from] std::io::Error),
    #[error("usage error")]
    UsageError(String),
    #[error("Nix parsing error")]
    NixParsingError(String),
    #[error("unknown error")]
    StringError(String),
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        return Error::StringError(s.to_string());
    }
}

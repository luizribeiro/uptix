#[derive(thiserror::Error, Debug)]
pub enum UptixError {
    #[error("registry error")]
    RegistryError(#[from] dkregistry::errors::Error),
    #[error("error")]
    StringError(String),
}

impl From<&str> for UptixError {
    fn from(s: &str) -> Self {
        return UptixError::StringError(s.to_string());
    }
}

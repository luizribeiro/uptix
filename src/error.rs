use thiserror::Error;

#[derive(Error, Debug)]
pub enum UptixError {
    #[error("error")]
    StringError(String),
}

impl From<&str> for UptixError {
    fn from(s: &str) -> Self {
        return UptixError::StringError(s.to_string());
    }
}

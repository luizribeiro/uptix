use miette::{Diagnostic, NamedSource, SourceSpan};

#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum Error {
    #[error("registry error")]
    #[diagnostic(code(uptix::error::registry))]
    RegistryError(#[from] dkregistry::errors::Error),
    #[error("HTTP request error")]
    #[diagnostic(code(uptix::error::request_error))]
    RequestError(#[from] reqwest::Error),
    #[error("URL construction error")]
    #[diagnostic(code(uptix::error::url_error))]
    URLConstructionError(#[from] url::ParseError),
    #[error("JSON serialization error")]
    #[diagnostic(code(uptix::error::json_parsing_error))]
    JSONParsingError(#[from] serde_json::Error),
    #[error("I/O error")]
    #[diagnostic(code(uptix::error::io_error))]
    IOError(#[from] std::io::Error),
    #[error("Nix parsing error")]
    #[diagnostic(code(uptix::error::nix_parsing_error))]
    NixParsingError(String),
    #[error("Unexpected argument for {function}")]
    #[diagnostic(help(
        "here are some examples of valid arguments:\n - homeassistant/home-assistant:stable"
    ))]
    UnexpectedArgument {
        function: String,
        #[source_code]
        src: NamedSource,
        #[label("expected a {expected_type} literal here")]
        argument_pos: SourceSpan,
        expected_type: String,
    },
    #[error("unknown error")]
    #[diagnostic(code(uptix::error::unknown_error))]
    StringError(String),
}

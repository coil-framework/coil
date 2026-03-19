use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("command `{path}` is already registered")]
    DuplicateCommand { path: String },
    #[error("command `{path}` was not found")]
    UnknownCommand { path: String },
    #[error("command `{path}` does not support --dry-run")]
    DryRunUnsupported { path: String },
    #[error("command `{path}` must be confirmed explicitly")]
    ConfirmationRequired { path: String },
}

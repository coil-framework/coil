use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidRoute { field: &'static str, value: String },
    DuplicateResource { resource_id: String },
    DuplicateWidget { widget_id: String },
    DuplicateWorkflow { workflow_id: String },
}

impl fmt::Display for AdminModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateResource { resource_id } => {
                write!(f, "admin resource `{resource_id}` is duplicated")
            }
            Self::DuplicateWidget { widget_id } => {
                write!(f, "admin widget `{widget_id}` is duplicated")
            }
            Self::DuplicateWorkflow { workflow_id } => {
                write!(f, "admin workflow `{workflow_id}` is duplicated")
            }
        }
    }
}

impl Error for AdminModelError {}

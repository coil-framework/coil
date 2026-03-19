use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum A11yError {
    EmptyField { field: &'static str },
    InvalidId { field: &'static str, value: String },
    InvalidContrastRatio { field: &'static str, ratio: f32 },
    MissingLabel { field_id: String },
    MissingCaption { table_id: String },
}

impl fmt::Display for A11yError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidId { field, value } => {
                write!(f, "`{field}` contains an invalid id `{value}`")
            }
            Self::InvalidContrastRatio { field, ratio } => {
                write!(f, "`{field}` must be at least 0 and finite, got `{ratio}`")
            }
            Self::MissingLabel { field_id } => {
                write!(f, "form field `{field_id}` is missing an accessible label")
            }
            Self::MissingCaption { table_id } => {
                write!(f, "table `{table_id}` is missing a caption")
            }
        }
    }
}

impl Error for A11yError {}

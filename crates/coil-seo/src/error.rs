use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeoError {
    EmptyField { field: &'static str },
    InvalidUrl { field: &'static str, value: String },
    InvalidJsonLdProperty { property: String },
    DuplicateJsonLdProperty { property: String },
}

impl fmt::Display for SeoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidUrl { field, value } => {
                write!(f, "`{field}` must be an absolute URL, got `{value}`")
            }
            Self::InvalidJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is invalid")
            }
            Self::DuplicateJsonLdProperty { property } => {
                write!(f, "JSON-LD property `{property}` is duplicated")
            }
        }
    }
}

impl Error for SeoError {}

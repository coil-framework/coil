use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidRoute { field: &'static str, value: String },
    DuplicateMessageKey { locale: String, key: String },
    MissingLocale { locale: String },
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateMessageKey { locale, key } => {
                write!(f, "locale `{locale}` duplicates translation key `{key}`")
            }
            Self::MissingLocale { locale } => {
                write!(
                    f,
                    "locale `{locale}` is not registered in the translation runtime"
                )
            }
        }
    }
}

impl Error for I18nError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationCatalogLoadError {
    Read { path: String, reason: String },
    Parse { path: String, reason: String },
}

impl fmt::Display for TranslationCatalogLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, reason } => {
                write!(f, "failed to read translation catalog `{path}`: {reason}")
            }
            Self::Parse { path, reason } => {
                write!(f, "failed to parse translation catalog `{path}`: {reason}")
            }
        }
    }
}

impl Error for TranslationCatalogLoadError {}

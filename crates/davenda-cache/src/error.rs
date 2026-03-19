use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    PublicScopeCannotVaryByUser,
    PublicScopeCannotVaryBySession,
    UncacheableApplicationScope,
    MissingHttpFreshness,
    NoStoreCannotDefineFreshness,
    ZeroDuration { field: &'static str },
    UnknownInflightFill { key: String },
}

impl fmt::Display for CacheModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::PublicScopeCannotVaryByUser => {
                f.write_str("public cache scopes cannot vary by user")
            }
            Self::PublicScopeCannotVaryBySession => {
                f.write_str("public cache scopes cannot vary by session")
            }
            Self::UncacheableApplicationScope => {
                f.write_str("application cache policy cannot use a no-store scope")
            }
            Self::MissingHttpFreshness => {
                f.write_str("cacheable HTTP responses must define a freshness policy")
            }
            Self::NoStoreCannotDefineFreshness => {
                f.write_str("no-store HTTP responses cannot define freshness")
            }
            Self::ZeroDuration { field } => write!(f, "`{field}` must be greater than zero"),
            Self::UnknownInflightFill { key } => {
                write!(f, "cache fill for `{key}` is not currently in progress")
            }
        }
    }
}

impl Error for CacheModelError {}

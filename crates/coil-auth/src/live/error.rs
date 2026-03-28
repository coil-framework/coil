use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveAuthError {
    ExplainApiDisabled,
    BackendInitialization { reason: String },
    Explain { reason: String },
}

impl fmt::Display for LiveAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplainApiDisabled => {
                write!(f, "auth explain API is disabled by deployment config")
            }
            Self::BackendInitialization { reason } => {
                write!(f, "failed to initialize the live auth backend: {reason}")
            }
            Self::Explain { reason } => {
                write!(f, "failed to explain capability: {reason}")
            }
        }
    }
}

impl Error for LiveAuthError {}

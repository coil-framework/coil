use super::*;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeAuthError {
    #[error("auth explain API is disabled by deployment config")]
    ExplainApiDisabled,
    #[error("failed to initialize the live auth backend: {reason}")]
    BackendInitialization { reason: String },
    #[error("failed to explain capability: {reason}")]
    Explain { reason: String },
}

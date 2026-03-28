use crate::CliModelError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliRunError {
    #[error(transparent)]
    Model(#[from] CliModelError),
    #[error("{message}")]
    Usage { message: String },
    #[error("{message}")]
    Execution { message: String },
}

impl CliRunError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage {
            message: message.into(),
        }
    }

    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage { .. } => 2,
            Self::Execution { .. } | Self::Model(_) => 1,
        }
    }
}

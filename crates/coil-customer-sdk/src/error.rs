use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorKind {
    InvalidInput,
    Unauthorized,
    Forbidden,
    Conflict,
    Unavailable,
    Timeout,
    Unsupported,
    Internal,
}

impl BackendErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::Unsupported => "unsupported",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    kind: BackendErrorKind,
    code: String,
    message: String,
    detail: Option<String>,
}

impl BackendError {
    pub fn new(
        kind: BackendErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub const fn kind(&self) -> BackendErrorKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(
                f,
                "{}:{}: {} ({detail})",
                self.kind.as_str(),
                self.code,
                self.message
            ),
            None => write!(f, "{}:{}: {}", self.kind.as_str(), self.code, self.message),
        }
    }
}

impl Error for BackendError {}

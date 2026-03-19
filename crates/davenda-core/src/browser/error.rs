use super::CookieProtection;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrowserSecurityError {
    #[error("browser security operations require a non-empty secret")]
    EmptySecret,
    #[error("cookie policy expects {expected:?} protection but was used as {actual:?}")]
    UnexpectedCookieProtection {
        expected: CookieProtection,
        actual: CookieProtection,
    },
    #[error("cookie value is not in the expected signed format")]
    InvalidCookieFormat,
    #[error("signed cookie failed verification")]
    InvalidCookieSignature,
    #[error("encrypted cookie is not in the expected format")]
    InvalidEncryptedCookieFormat,
    #[error("encrypted cookie failed decryption")]
    InvalidEncryptedCookiePayload,
    #[error("CSRF protection is disabled for this runtime")]
    CsrfDisabled,
    #[error("CSRF token is not in the expected format")]
    InvalidCsrfTokenFormat,
}

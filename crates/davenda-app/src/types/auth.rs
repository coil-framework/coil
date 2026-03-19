use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Extend,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStrategy {
    pub mode: AuthMode,
    pub package_name: String,
}

impl AuthStrategy {
    pub fn new(mode: AuthMode, package_name: impl Into<String>) -> Result<Self, AppModelError> {
        Ok(Self {
            mode,
            package_name: require_non_empty("auth_package_name", package_name.into())?,
        })
    }
}

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionPrincipal {
    Anonymous,
    User(String),
    ServiceAccount(String),
}

impl ExtensionPrincipal {
    pub fn anonymous() -> Self {
        Self::Anonymous
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self::User(id.into())
    }

    pub fn service_account(id: impl Into<String>) -> Self {
        Self::ServiceAccount(id.into())
    }

    pub(super) fn to_wasm_principal(&self) -> Result<PrincipalRef, WasmModelError> {
        match self {
            Self::Anonymous => Ok(PrincipalRef::anonymous()),
            Self::User(id) => PrincipalRef::user(id.clone()),
            Self::ServiceAccount(id) => PrincipalRef::service_account(id.clone()),
        }
    }
}

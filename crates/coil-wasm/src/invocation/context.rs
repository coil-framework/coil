use std::collections::BTreeMap;

use crate::error::WasmModelError;

use super::InvocationInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppContext {
    pub app_id: String,
    pub tenant_id: Option<String>,
    pub site_id: Option<String>,
    pub brand_id: Option<String>,
    pub locale: Option<String>,
}

impl CustomerAppContext {
    pub fn new(app_id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            app_id: crate::validation::validate_token("app_id", app_id.into())?,
            tenant_id: None,
            site_id: None,
            brand_id: None,
            locale: None,
        })
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.tenant_id = Some(crate::validation::validate_token(
            "tenant_id",
            tenant_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_site_id(mut self, site_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.site_id = Some(crate::validation::validate_token(
            "site_id",
            site_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_brand_id(mut self, brand_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.brand_id = Some(crate::validation::validate_token(
            "brand_id",
            brand_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, WasmModelError> {
        self.locale = Some(crate::validation::validate_token("locale", locale.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    Anonymous,
    User,
    ServiceAccount,
}

impl std::fmt::Display for PrincipalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anonymous => f.write_str("anonymous"),
            Self::User => f.write_str("user"),
            Self::ServiceAccount => f.write_str("service_account"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalRef {
    pub kind: PrincipalKind,
    pub id: Option<String>,
}

impl PrincipalRef {
    pub fn anonymous() -> Self {
        Self {
            kind: PrincipalKind::Anonymous,
            id: None,
        }
    }

    pub fn user(id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            kind: PrincipalKind::User,
            id: Some(crate::validation::validate_token(
                "principal_id",
                id.into(),
            )?),
        })
    }

    pub fn service_account(id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            kind: PrincipalKind::ServiceAccount,
            id: Some(crate::validation::validate_token(
                "principal_id",
                id.into(),
            )?),
        })
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        match self.kind {
            PrincipalKind::Anonymous => Ok(()),
            PrincipalKind::User | PrincipalKind::ServiceAccount => {
                if self.id.as_deref().is_some_and(|id| !id.is_empty()) {
                    Ok(())
                } else {
                    Err(WasmModelError::PrincipalIdRequired { kind: self.kind })
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub request_id: Option<String>,
}

impl TraceContext {
    pub fn new(trace_id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            trace_id: crate::validation::validate_token("trace_id", trace_id.into())?,
            parent_span_id: None,
            request_id: None,
        })
    }

    pub fn with_parent_span_id(
        mut self,
        parent_span_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.parent_span_id = Some(crate::validation::validate_token(
            "parent_span_id",
            parent_span_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_request_id(
        mut self,
        request_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.request_id = Some(crate::validation::validate_token(
            "request_id",
            request_id.into(),
        )?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationContext {
    pub customer_app: CustomerAppContext,
    pub principal: PrincipalRef,
    pub trace: TraceContext,
    pub extension_config: BTreeMap<String, crate::manifest::ExtensionConfigValue>,
    pub input: InvocationInput,
}

impl InvocationContext {
    pub fn new(
        customer_app: CustomerAppContext,
        principal: PrincipalRef,
        trace: TraceContext,
        input: InvocationInput,
    ) -> Self {
        Self {
            customer_app,
            principal,
            trace,
            extension_config: BTreeMap::new(),
            input,
        }
    }

    pub fn with_config_value(
        mut self,
        key: impl Into<String>,
        value: crate::manifest::ExtensionConfigValue,
    ) -> Result<Self, WasmModelError> {
        let key = crate::validation::validate_token("extension_config_key", key.into())?;
        value.validate_for_key(&key)?;
        self.extension_config.insert(key, value);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        self.principal.validate()?;
        for (key, value) in &self.extension_config {
            crate::validation::validate_token("extension_config_key", key.clone())?;
            value.validate_for_key(key)?;
        }
        Ok(())
    }
}

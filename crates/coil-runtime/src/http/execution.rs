use super::*;
use crate::{FlashMessage, RuntimeBrowserError};
use coil_cache::{CacheModelError, CachePlan};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

pub type RequestFieldMap = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInput {
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub query_params: RequestFieldMap,
    pub form_fields: RequestFieldMap,
    pub content_type: Option<String>,
    pub raw_body: Vec<u8>,
    pub scheme: String,
    pub forwarded_proto: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub session_cookie: Option<String>,
    pub flash_cookie: Option<String>,
    pub csrf_token: Option<String>,
    pub csrf_action: Option<String>,
    pub maintenance_bypass_token: Option<String>,
    pub principal_id: Option<String>,
    pub principal_kind: RequestPrincipalKind,
    pub granted_capabilities: HashSet<coil_auth::Capability>,
}

impl RequestInput {
    pub fn new(
        method: HttpMethod,
        host: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            method,
            host: validate_host(host.into())?,
            path: validate_route_path(path.into())?,
            headers: BTreeMap::new(),
            query_params: RequestFieldMap::new(),
            form_fields: RequestFieldMap::new(),
            content_type: None,
            raw_body: Vec::new(),
            scheme: "https".to_string(),
            forwarded_proto: None,
            request_id: None,
            session_id: None,
            session_cookie: None,
            flash_cookie: None,
            csrf_token: None,
            csrf_action: None,
            maintenance_bypass_token: None,
            principal_id: None,
            principal_kind: RequestPrincipalKind::Anonymous,
            granted_capabilities: HashSet::new(),
        })
    }

    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = scheme.into();
        self
    }

    pub fn with_forwarded_proto(mut self, proto: impl Into<String>) -> Self {
        self.forwarded_proto = Some(proto.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_session_cookie(mut self, session_cookie: impl Into<String>) -> Self {
        self.session_cookie = Some(session_cookie.into());
        self
    }

    pub fn with_flash_cookie(mut self, flash_cookie: impl Into<String>) -> Self {
        self.flash_cookie = Some(flash_cookie.into());
        self
    }

    pub fn with_csrf_token(mut self, csrf_token: impl Into<String>) -> Self {
        self.csrf_token = Some(csrf_token.into());
        self
    }

    pub fn with_csrf_action(mut self, csrf_action: impl Into<String>) -> Self {
        self.csrf_action = Some(csrf_action.into());
        self
    }

    pub fn with_maintenance_bypass_token(mut self, bypass_token: impl Into<String>) -> Self {
        self.maintenance_bypass_token = Some(bypass_token.into());
        self
    }

    pub fn with_principal(mut self, principal_id: impl Into<String>) -> Self {
        self.principal_id = Some(principal_id.into());
        self.principal_kind = RequestPrincipalKind::User;
        self
    }

    pub fn with_service_account_principal(mut self, principal_id: impl Into<String>) -> Self {
        self.principal_id = Some(principal_id.into());
        self.principal_kind = RequestPrincipalKind::ServiceAccount;
        self
    }

    pub fn with_query_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        push_request_field(&mut self.query_params, name.into(), value.into());
        self
    }

    pub fn with_form_field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        push_request_field(&mut self.form_fields, name.into(), value.into());
        self
    }

    pub fn with_query_params(mut self, params: RequestFieldMap) -> Self {
        for (name, values) in params {
            for value in values {
                push_request_field(&mut self.query_params, name.clone(), value);
            }
        }
        self
    }

    pub fn with_form_fields(mut self, fields: RequestFieldMap) -> Self {
        for (name, values) in fields {
            for value in values {
                push_request_field(&mut self.form_fields, name.clone(), value);
            }
        }
        self
    }

    pub fn with_headers(mut self, headers: BTreeMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_raw_body(mut self, raw_body: Vec<u8>) -> Self {
        self.raw_body = raw_body;
        self
    }

    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query_params
            .get(name)
            .and_then(|values| values.first().map(String::as_str))
    }

    pub fn form_field(&self, name: &str) -> Option<&str> {
        self.form_fields
            .get(name)
            .and_then(|values| values.first().map(String::as_str))
    }

    pub fn grant_capability(mut self, capability: coil_auth::Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }
}

pub(crate) fn push_request_field(fields: &mut RequestFieldMap, name: String, value: String) {
    if !request_field_name_is_valid(&name) || !request_field_value_is_valid(&value) {
        return;
    }

    fields.entry(name).or_default().push(value);
}

fn request_field_name_is_valid(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(|ch| ch.is_control())
}

fn request_field_value_is_valid(value: &str) -> bool {
    !value
        .chars()
        .any(|ch| ch == '\0' || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPrincipalKind {
    Anonymous,
    User,
    ServiceAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTraceContext {
    pub request_id: String,
    pub transport_scheme: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContext {
    pub session_id: Option<String>,
    pub resolved_from_cookie: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalContext {
    pub principal_id: Option<String>,
    pub principal_kind: RequestPrincipalKind,
    pub granted_capabilities: HashSet<coil_auth::Capability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheDisposition {
    Public,
    Private,
    Uncacheable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestExecution {
    pub customer_app: String,
    pub site_id: Option<String>,
    pub site_display_name: Option<String>,
    pub brand_name: Option<String>,
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub query_params: RequestFieldMap,
    pub form_fields: RequestFieldMap,
    pub content_type: Option<String>,
    pub raw_body: Vec<u8>,
    pub route: ResolvedRoute,
    pub route_area: RouteArea,
    pub locale: String,
    pub trace: RequestTraceContext,
    pub session: SessionContext,
    pub principal: PrincipalContext,
    pub cache: CacheDisposition,
    pub cache_plan: ExecutedCachePlan,
    pub middleware: Vec<MiddlewareStage>,
    pub response: HandlerResponse,
    pub flash_messages: Vec<FlashMessage>,
    pub response_cookies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedCachePlan {
    pub plan: CachePlan,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RequestExecutionError {
    #[error("no route matches {method:?} {host}{path}")]
    RouteNotFound {
        method: HttpMethod,
        host: String,
        path: String,
    },
    #[error("route `{route}` requires a resolved session")]
    SessionRequired { route: String },
    #[error("route `{route}` requires capability `{capability}`")]
    CapabilityRequired {
        route: String,
        capability: coil_auth::Capability,
    },
    #[error("route `{route}` requires a CSRF token")]
    MissingCsrfToken { route: String },
    #[error("route `{route}` requires a session before CSRF can be validated")]
    MissingSessionForCsrf { route: String },
    #[error("route `{route}` supplied an invalid CSRF token")]
    InvalidCsrfToken { route: String },
    #[error("session cookie failed validation: {0}")]
    InvalidSessionCookie(String),
    #[error("flash cookie failed validation: {0}")]
    InvalidFlashCookie(String),
    #[error("session `{session_id}` is not present in the server-side store")]
    UnknownSession { session_id: String },
    #[error("session `{session_id}` has expired")]
    ExpiredSession { session_id: String },
    #[error("session `{session_id}` has been revoked")]
    RevokedSession { session_id: String },
    #[error("route `{route}` is disabled by maintenance mode")]
    MaintenanceMode { route: String },
    #[error("route `{route}` is disabled because feature flag `{feature_flag}` is not enabled")]
    FeatureFlagDisabled { route: String, feature_flag: String },
    #[error("route `{route}` has no registered handler")]
    HandlerNotRegistered { route: String },
    #[error(transparent)]
    Cache(#[from] CacheModelError),
}

impl RequestExecutionError {
    pub(crate) fn from_browser_error(error: RuntimeBrowserError) -> Self {
        match error {
            RuntimeBrowserError::InvalidSessionCookie { reason } => {
                Self::InvalidSessionCookie(reason)
            }
            RuntimeBrowserError::InvalidFlashCookie { reason } => Self::InvalidFlashCookie(reason),
            RuntimeBrowserError::UnknownSession { session_id } => {
                Self::UnknownSession { session_id }
            }
            RuntimeBrowserError::ExpiredSession { session_id } => {
                Self::ExpiredSession { session_id }
            }
            RuntimeBrowserError::RevokedSession { session_id } => {
                Self::RevokedSession { session_id }
            }
            other => Self::InvalidFlashCookie(other.to_string()),
        }
    }
}

use super::*;
use crate::{FlashMessage, RuntimeBrowserError};
use davenda_cache::{CacheModelError, CachePlan};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInput {
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
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
    pub granted_capabilities: HashSet<davenda_auth::Capability>,
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
        self
    }

    pub fn grant_capability(mut self, capability: davenda_auth::Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }
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
    pub granted_capabilities: HashSet<davenda_auth::Capability>,
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
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
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
        capability: davenda_auth::Capability,
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

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const fn is_state_changing(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteArea {
    Public,
    Account,
    Admin,
    Api,
    Fragment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    Any,
    Exact(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalePolicy {
    DefaultOnly,
    Localized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAuthGate {
    Public,
    Session,
    Capability(davenda_auth::Capability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDefinition {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
    pub area: RouteArea,
    pub host: HostPattern,
    pub locale_policy: LocalePolicy,
    pub auth: RouteAuthGate,
    pub module: Option<String>,
    pub feature_flag: Option<String>,
}

impl RouteDefinition {
    pub fn new(
        name: impl Into<String>,
        method: HttpMethod,
        path: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        let name = validate_route_name(name.into())?;
        let path = validate_route_path(path.into())?;

        Ok(Self {
            name,
            method,
            path,
            area: RouteArea::Public,
            host: HostPattern::Any,
            locale_policy: LocalePolicy::DefaultOnly,
            auth: RouteAuthGate::Public,
            module: None,
            feature_flag: None,
        })
    }

    pub fn with_area(mut self, area: RouteArea) -> Self {
        self.area = area;
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Result<Self, RouteBuildError> {
        self.host = HostPattern::Exact(validate_host(host.into())?);
        Ok(self)
    }

    pub fn localized(mut self) -> Self {
        self.locale_policy = LocalePolicy::Localized;
        self
    }

    pub fn requiring_session(mut self) -> Self {
        self.auth = RouteAuthGate::Session;
        self
    }

    pub fn requiring_capability(mut self, capability: davenda_auth::Capability) -> Self {
        self.auth = RouteAuthGate::Capability(capability);
        self
    }

    pub fn from_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: impl Into<String>) -> Self {
        self.feature_flag = Some(feature_flag.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddlewareStage {
    TransportNormalization,
    CustomerAppResolution,
    TraceContext,
    LocaleResolution,
    SessionResolution,
    BrowserPolicy,
    ResponsePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRuntimePlan {
    pub middleware: Vec<MiddlewareStage>,
    pub routes: Vec<RouteDefinition>,
}

impl HttpRuntimePlan {
    pub fn resolve(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRoute> {
        self.resolve_match(config, method, host, path)
            .map(|matched| matched.resolved)
    }

    pub fn resolve_match(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRouteMatch> {
        self.routes.iter().find_map(|route| {
            if route.method != method {
                return None;
            }

            if let HostPattern::Exact(expected) = &route.host {
                if expected != host {
                    return None;
                }
            }

            match route.locale_policy {
                LocalePolicy::DefaultOnly => match match_route_path(&route.path, path) {
                    Some(params) => Some(ResolvedRouteMatch {
                        route: route.clone(),
                        resolved: ResolvedRoute {
                            route_name: route.name.clone(),
                            locale: None,
                            auth: route.auth,
                            params,
                        },
                    }),
                    None => None,
                },
                LocalePolicy::Localized if config.i18n.localized_routes => {
                    config.i18n.supported_locales.iter().find_map(|locale| {
                        let localized_path = format!(
                            "/{}/{}",
                            locale.trim_matches('/'),
                            route.path.trim_start_matches('/')
                        );
                        match_route_path(&localized_path, path).map(|params| ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                locale: Some(locale.clone()),
                                auth: route.auth,
                                params,
                            },
                        })
                    })
                }
                LocalePolicy::Localized => None,
            }
        })
    }

    pub fn path_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let rendered_path = render_route_path(&route.path, params, route_name)?;

        if route.locale_policy == LocalePolicy::Localized {
            let locale = locale.unwrap_or(&config.i18n.default_locale);
            if !config
                .i18n
                .supported_locales
                .iter()
                .any(|item| item == locale)
            {
                return Err(RouteUrlError::UnsupportedLocale {
                    route: route_name.to_string(),
                    locale: locale.to_string(),
                });
            }

            return Ok(format!(
                "/{}/{}",
                locale.trim_matches('/'),
                rendered_path.trim_start_matches('/')
            ));
        }

        Ok(rendered_path)
    }

    pub fn absolute_url_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let path = self.path_for(config, route_name, params, locale)?;
        let host = match &route.host {
            HostPattern::Exact(host) => host.as_str(),
            HostPattern::Any => config.seo.canonical_host.as_str(),
        };
        Ok(format!("https://{host}{path}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route_name: String,
    pub locale: Option<String>,
    pub auth: RouteAuthGate,
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRouteMatch {
    pub route: RouteDefinition,
    pub resolved: ResolvedRoute,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageResponse {
    pub template: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentResponse {
    pub template: String,
    pub fragment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectResponse {
    pub location: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonResponse {
    pub status: u16,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResponse {
    pub logical_path: String,
    pub content_type: String,
    pub delivery_mode: FileDeliveryMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerResponse {
    Page(PageResponse),
    Fragment(FragmentResponse),
    Redirect(RedirectResponse),
    Json(JsonResponse),
    File(FileResponse),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerDefinition {
    pub route_name: String,
    pub response: HandlerResponse,
}

impl HandlerDefinition {
    pub fn page(
        route_name: impl Into<String>,
        template: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Page(PageResponse {
                template: validate_template_name(template.into())?,
                status: 200,
            }),
        })
    }

    pub fn fragment(
        route_name: impl Into<String>,
        template: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Fragment(FragmentResponse {
                template: validate_template_name(template.into())?,
                fragment_id: validate_fragment_id(fragment_id.into())?,
            }),
        })
    }

    pub fn redirect(
        route_name: impl Into<String>,
        location: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Redirect(RedirectResponse {
                location: validate_route_path(location.into())?,
                status: 303,
            }),
        })
    }

    pub fn json(
        route_name: impl Into<String>,
        payload: BTreeMap<String, String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Json(JsonResponse {
                status: 200,
                payload,
            }),
        })
    }

    pub fn file(
        route_name: impl Into<String>,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: FileDeliveryMode,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::File(FileResponse {
                logical_path: validate_template_name(logical_path.into())?,
                content_type: validate_template_name(content_type.into())?,
                delivery_mode,
            }),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteBuildError {
    #[error("route names must not be empty")]
    EmptyRouteName,
    #[error("route paths must start with `/`, got `{path}`")]
    InvalidRoutePath { path: String },
    #[error("host pattern must not be empty")]
    EmptyHostPattern,
    #[error("route `{name}` is registered more than once for method {method:?}")]
    DuplicateRoute { name: String, method: HttpMethod },
    #[error(
        "route `{name}` requires capability `{capability}` but the auth package does not bind it"
    )]
    MissingCapabilityBinding {
        name: String,
        capability: davenda_auth::Capability,
    },
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteUrlError {
    #[error("route `{route}` is not registered")]
    UnknownRoute { route: String },
    #[error("route `{route}` requires parameter `{parameter}`")]
    MissingRouteParameter { route: String, parameter: String },
    #[error("route `{route}` does not support locale `{locale}`")]
    UnsupportedLocale { route: String, locale: String },
}

fn match_route_path(pattern: &str, actual: &str) -> Option<BTreeMap<String, String>> {
    let pattern_segments = path_segments(pattern);
    let actual_segments = path_segments(actual);
    if pattern_segments.len() != actual_segments.len() {
        return None;
    }

    let mut params = BTreeMap::new();
    for (pattern_segment, actual_segment) in pattern_segments.iter().zip(actual_segments.iter()) {
        if pattern_segment.starts_with('{')
            && pattern_segment.ends_with('}')
            && pattern_segment.len() > 2
        {
            params.insert(
                pattern_segment[1..pattern_segment.len() - 1].to_string(),
                (*actual_segment).to_string(),
            );
        } else if pattern_segment != actual_segment {
            return None;
        }
    }

    Some(params)
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn render_route_path(
    pattern: &str,
    params: &BTreeMap<String, String>,
    route_name: &str,
) -> Result<String, RouteUrlError> {
    let rendered_segments = path_segments(pattern)
        .into_iter()
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2 {
                let parameter = &segment[1..segment.len() - 1];
                params
                    .get(parameter)
                    .cloned()
                    .ok_or_else(|| RouteUrlError::MissingRouteParameter {
                        route: route_name.to_string(),
                        parameter: parameter.to_string(),
                    })
            } else {
                Ok(segment.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if rendered_segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", rendered_segments.join("/")))
    }
}

fn validate_route_name(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyRouteName)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_route_path(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.starts_with('/') && !trimmed.is_empty() {
        Ok(trimmed.to_string())
    } else {
        Err(RouteBuildError::InvalidRoutePath {
            path: value.trim().to_string(),
        })
    }
}

fn validate_host(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyHostPattern)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_template_name(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}

fn validate_fragment_id(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}

use std::collections::HashSet;

use davenda_auth::AuthModelPackage;
use davenda_cache::CacheTopology;
use davenda_config::{ConfigError, PlatformConfig};
use davenda_core::{
    BrowserSecurityServices, CapabilityValidationError, CookieSigner, ModuleManifest,
    PlatformModule, RegistrationError, ServiceDescriptor, TemplateRuntimeServices,
    WasmRuntimeServices, bootstrap_core_services, validate_module_capabilities,
};
use thiserror::Error;

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
                LocalePolicy::DefaultOnly => {
                    if route.path == path {
                        Some(ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                locale: None,
                                auth: route.auth,
                            },
                        })
                    } else {
                        None
                    }
                }
                LocalePolicy::Localized if config.i18n.localized_routes => {
                    config.i18n.supported_locales.iter().find_map(|locale| {
                        let localized_path = format!(
                            "/{}/{}",
                            locale.trim_matches('/'),
                            route.path.trim_start_matches('/')
                        );
                        (localized_path == path).then(|| ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                locale: Some(locale.clone()),
                                auth: route.auth,
                            },
                        })
                    })
                }
                LocalePolicy::Localized => None,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route_name: String,
    pub locale: Option<String>,
    pub auth: RouteAuthGate,
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
    pub csrf_token: Option<String>,
    pub csrf_action: Option<String>,
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
            csrf_token: None,
            csrf_action: None,
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

    pub fn with_csrf_token(mut self, csrf_token: impl Into<String>) -> Self {
        self.csrf_token = Some(csrf_token.into());
        self
    }

    pub fn with_csrf_action(mut self, csrf_action: impl Into<String>) -> Self {
        self.csrf_action = Some(csrf_action.into());
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
    pub route: ResolvedRoute,
    pub route_area: RouteArea,
    pub locale: String,
    pub trace: RequestTraceContext,
    pub session: SessionContext,
    pub principal: PrincipalContext,
    pub cache: CacheDisposition,
    pub middleware: Vec<MiddlewareStage>,
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
}

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
    routes: Vec<RouteDefinition>,
}

impl<P> RuntimeBuilder<P>
where
    P: AuthModelPackage,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            config,
            auth_package,
            modules: Vec::new(),
            routes: Vec::new(),
        }
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn with_route(mut self, route: RouteDefinition) -> Self {
        self.routes.push(route);
        self
    }

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        self.config.validate().map_err(ConfigError::Validation)?;

        if self.auth_package.manifest().name != self.config.auth.package {
            return Err(RuntimeBuildError::AuthPackageMismatch {
                configured: self.config.auth.package,
                actual: self.auth_package.manifest().name.clone(),
            });
        }

        let bootstrap = bootstrap_core_services(&self.config)?;
        let mut registry = bootstrap.registry;
        let mut module_manifests = Vec::new();
        let http = build_http_runtime_plan(&self.auth_package, &self.routes)?;

        for module in self.modules {
            let manifest = module.manifest();
            validate_module_capabilities(&self.auth_package, &manifest)?;
            registry.register_module_manifest(manifest.clone())?;
            module.register(&mut registry)?;
            module_manifests.push(manifest);
        }

        Ok(RuntimePlan {
            config: self.config,
            auth_package_name: self.auth_package.manifest().name.clone(),
            cache_topology: bootstrap.cache.topology,
            browser: bootstrap.browser,
            http,
            template: bootstrap.template,
            wasm: bootstrap.wasm,
            services: registry.services().cloned().collect(),
            modules: module_manifests,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub cache_topology: CacheTopology,
    pub browser: BrowserSecurityServices,
    pub http: HttpRuntimePlan,
    pub template: TemplateRuntimeServices,
    pub wasm: WasmRuntimeServices,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
}

impl RuntimePlan {
    pub fn execute_request(
        &self,
        request: RequestInput,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<RequestExecution, RequestExecutionError> {
        let matched = self
            .http
            .resolve_match(&self.config, request.method, &request.host, &request.path)
            .ok_or_else(|| RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })?;

        let trace = RequestTraceContext {
            request_id: request.request_id.clone().unwrap_or_else(|| {
                format!(
                    "req:{:?}:{}:{}",
                    request.method, request.host, matched.resolved.route_name
                )
            }),
            transport_scheme: request
                .forwarded_proto
                .clone()
                .unwrap_or_else(|| request.scheme.clone()),
        };

        let mut resolved_from_cookie = false;
        let session_id = if let Some(session_id) = request.session_id.clone() {
            Some(session_id)
        } else if let Some(cookie) = request.session_cookie.as_deref() {
            resolved_from_cookie = true;
            Some(self.verify_session_cookie(cookie_secret, cookie)?)
        } else {
            None
        };

        let session = SessionContext {
            session_id: session_id.clone(),
            resolved_from_cookie,
        };
        let principal = PrincipalContext {
            principal_id: request.principal_id,
            granted_capabilities: request.granted_capabilities,
        };

        self.enforce_route_auth(&matched.resolved, &session, &principal)?;
        self.enforce_browser_policy(
            &matched.route,
            &matched.resolved,
            request.method,
            request.csrf_action.as_deref(),
            request.csrf_token.as_deref(),
            &session,
            csrf_secret,
        )?;

        Ok(RequestExecution {
            customer_app: self.config.app.name.clone(),
            route: matched.resolved.clone(),
            route_area: matched.route.area,
            locale: matched
                .resolved
                .locale
                .clone()
                .unwrap_or_else(|| self.config.i18n.default_locale.clone()),
            trace,
            session: session.clone(),
            principal,
            cache: cache_disposition_for_route(request.method, &matched.resolved.auth, &session),
            middleware: self.http.middleware.clone(),
        })
    }

    fn verify_session_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<String, RequestExecutionError> {
        let signer = CookieSigner::new(self.browser.sessions.session_cookie.clone());
        signer
            .verify(cookie_secret, cookie)
            .map_err(|error| RequestExecutionError::InvalidSessionCookie(error.to_string()))
    }

    fn enforce_route_auth(
        &self,
        route: &ResolvedRoute,
        session: &SessionContext,
        principal: &PrincipalContext,
    ) -> Result<(), RequestExecutionError> {
        match route.auth {
            RouteAuthGate::Public => Ok(()),
            RouteAuthGate::Session => {
                if session.session_id.is_some() {
                    Ok(())
                } else {
                    Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    })
                }
            }
            RouteAuthGate::Capability(capability) => {
                if session.session_id.is_none() {
                    return Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    });
                }

                if principal.granted_capabilities.contains(&capability) {
                    Ok(())
                } else {
                    Err(RequestExecutionError::CapabilityRequired {
                        route: route.route_name.clone(),
                        capability,
                    })
                }
            }
        }
    }

    fn enforce_browser_policy(
        &self,
        route: &RouteDefinition,
        resolved: &ResolvedRoute,
        method: HttpMethod,
        csrf_action: Option<&str>,
        csrf_token: Option<&str>,
        session: &SessionContext,
        csrf_secret: &[u8],
    ) -> Result<(), RequestExecutionError> {
        let requires_csrf = method.is_state_changing()
            && route.area != RouteArea::Api
            && self.browser.csrf.enabled
            && !matches!(resolved.auth, RouteAuthGate::Public);

        if !requires_csrf {
            return Ok(());
        }

        let session_id = session.session_id.as_deref().ok_or_else(|| {
            RequestExecutionError::MissingSessionForCsrf {
                route: resolved.route_name.clone(),
            }
        })?;
        let token = csrf_token.ok_or_else(|| RequestExecutionError::MissingCsrfToken {
            route: resolved.route_name.clone(),
        })?;
        let action = csrf_action.unwrap_or(&resolved.route_name);
        let valid = self
            .browser
            .csrf
            .verify_token(csrf_secret, session_id, action, token)
            .map_err(|_| RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })?;

        if valid {
            Ok(())
        } else {
            Err(RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })
        }
    }
}

fn cache_disposition_for_route(
    method: HttpMethod,
    auth: &RouteAuthGate,
    session: &SessionContext,
) -> CacheDisposition {
    if method.is_state_changing() {
        return CacheDisposition::Uncacheable;
    }

    match auth {
        RouteAuthGate::Public if session.session_id.is_none() => CacheDisposition::Public,
        _ => CacheDisposition::Private,
    }
}

#[derive(Debug, Error)]
pub enum RuntimeBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error(transparent)]
    Route(#[from] RouteBuildError),
    #[error("configured auth package `{configured}` does not match loaded package `{actual}`")]
    AuthPackageMismatch { configured: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_cache::DistributedCacheBackend;
    use davenda_cms::CmsModule;
    use davenda_commerce::CommerceModule;
    use davenda_core::CookieSigner;
    use davenda_events::EventsModule;
    use davenda_memberships::MembershipsModule;
    use davenda_template::TemplateNamespace;
    use davenda_wasm::ExtensionPointKind;
    use std::time::Duration;

    const VALID_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

[modules]
enabled = ["cms-pages", "admin-shell"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

    #[test]
    fn runtime_builder_creates_a_runtime_plan() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("events.show", HttpMethod::Get, "/events")
                    .unwrap()
                    .localized()
                    .from_module("events"),
            )
            .with_route(
                RouteDefinition::new("admin.dashboard", HttpMethod::Get, "/admin")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_session(),
            )
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .with_module(EventsModule::new())
            .build()
            .unwrap();

        assert_eq!(plan.auth_package_name, "platform-default-auth");
        assert_eq!(
            plan.cache_topology.l2(),
            Some(DistributedCacheBackend::Redis)
        );
        assert_eq!(plan.browser.sessions.session_cookie.name, "davenda_session");
        assert_eq!(plan.browser.csrf.field_name, "_csrf");
        assert_eq!(
            plan.http.middleware,
            vec![
                MiddlewareStage::TransportNormalization,
                MiddlewareStage::CustomerAppResolution,
                MiddlewareStage::TraceContext,
                MiddlewareStage::LocaleResolution,
                MiddlewareStage::SessionResolution,
                MiddlewareStage::BrowserPolicy,
                MiddlewareStage::ResponsePolicy,
            ]
        );
        assert_eq!(
            plan.http.resolve(
                &plan.config,
                HttpMethod::Get,
                "www.example.com",
                "/fr-FR/events"
            ),
            Some(ResolvedRoute {
                route_name: "events.show".to_string(),
                locale: Some("fr-FR".to_string()),
                auth: RouteAuthGate::Public,
            })
        );
        assert_eq!(
            plan.template
                .namespace_chain(Some(&TemplateNamespace::new("cms-pages").unwrap())),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("cms-pages").unwrap(),
                TemplateNamespace::new("core").unwrap(),
            ]
        );
        assert_eq!(plan.wasm.extension_directory, "extensions");
        assert_eq!(
            plan.wasm
                .limits
                .for_point(ExtensionPointKind::Page)
                .max_runtime,
            Duration::from_millis(50)
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.cms.pages")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.commerce.checkout")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.memberships.entitlements")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.events.bookings")
        );
        assert_eq!(plan.modules.len(), 4);
        assert_eq!(plan.modules[0].name, "cms");
        assert_eq!(plan.modules[1].name, "commerce");
        assert_eq!(plan.modules[2].name, "memberships");
        assert_eq!(plan.modules[3].name, "events");
    }

    #[test]
    fn rejects_duplicate_route_names_for_the_same_method() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(RouteDefinition::new("events.show", HttpMethod::Get, "/events").unwrap())
            .with_route(
                RouteDefinition::new("events.show", HttpMethod::Get, "/events-duplicate").unwrap(),
            )
            .build()
            .unwrap_err();

        match error {
            RuntimeBuildError::Route(RouteBuildError::DuplicateRoute { name, method }) => {
                assert_eq!(name, "events.show");
                assert_eq!(method, HttpMethod::Get);
            }
            other => panic!("expected duplicate route error, got {other:?}"),
        }
    }

    #[test]
    fn execute_request_derives_context_and_session_from_cookie() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                    .unwrap()
                    .with_area(RouteArea::Account)
                    .requiring_session(),
            )
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/account")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .with_principal("user-1"),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(execution.customer_app, "showcase-events");
        assert_eq!(execution.route.route_name, "account.dashboard");
        assert_eq!(execution.route_area, RouteArea::Account);
        assert_eq!(execution.locale, "en-GB");
        assert_eq!(execution.session.session_id.as_deref(), Some("session-123"));
        assert!(execution.session.resolved_from_cookie);
        assert_eq!(execution.cache, CacheDisposition::Private);
        assert_eq!(execution.trace.transport_scheme, "https");
        assert_eq!(execution.middleware, plan.http.middleware);
    }

    #[test]
    fn execute_request_enforces_csrf_for_state_changing_browser_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("cms.publish", HttpMethod::Post, "/admin/pages/publish")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_session(),
            )
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let missing_token = plan.execute_request(
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                .unwrap()
                .with_session_cookie(session_cookie.clone()),
            cookie_secret,
            csrf_secret,
        );
        assert_eq!(
            missing_token.unwrap_err(),
            RequestExecutionError::MissingCsrfToken {
                route: "cms.publish".to_string(),
            }
        );

        let token = plan
            .browser
            .csrf
            .issue_token(csrf_secret, "session-123", "cms.publish")
            .unwrap();
        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .with_csrf_token(token),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(execution.cache, CacheDisposition::Uncacheable);
        assert_eq!(execution.route.route_name, "cms.publish");
    }

    #[test]
    fn execute_request_requires_capability_for_capability_gated_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("cms.preview", HttpMethod::Get, "/admin/pages/preview")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_capability(davenda_auth::Capability::CmsPageRead),
            )
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let denied = plan.execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
                .unwrap()
                .with_session_cookie(session_cookie.clone()),
            cookie_secret,
            csrf_secret,
        );
        assert_eq!(
            denied.unwrap_err(),
            RequestExecutionError::CapabilityRequired {
                route: "cms.preview".to_string(),
                capability: davenda_auth::Capability::CmsPageRead,
            }
        );

        let allowed = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .grant_capability(davenda_auth::Capability::CmsPageRead),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(allowed.route.route_name, "cms.preview");
        assert_eq!(allowed.cache, CacheDisposition::Private);
    }
}

fn build_http_runtime_plan<P>(
    package: &P,
    routes: &[RouteDefinition],
) -> Result<HttpRuntimePlan, RouteBuildError>
where
    P: AuthModelPackage,
{
    let mut seen = std::collections::BTreeSet::new();
    for route in routes {
        if !seen.insert((route.name.clone(), route.method)) {
            return Err(RouteBuildError::DuplicateRoute {
                name: route.name.clone(),
                method: route.method,
            });
        }

        if let RouteAuthGate::Capability(capability) = route.auth {
            if package.binding_for(capability).is_none() {
                return Err(RouteBuildError::MissingCapabilityBinding {
                    name: route.name.clone(),
                    capability,
                });
            }
        }
    }

    Ok(HttpRuntimePlan {
        middleware: vec![
            MiddlewareStage::TransportNormalization,
            MiddlewareStage::CustomerAppResolution,
            MiddlewareStage::TraceContext,
            MiddlewareStage::LocaleResolution,
            MiddlewareStage::SessionResolution,
            MiddlewareStage::BrowserPolicy,
            MiddlewareStage::ResponsePolicy,
        ],
        routes: routes.to_vec(),
    })
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

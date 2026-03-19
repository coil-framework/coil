use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{COOKIE, HOST, LOCATION};
use axum::http::{HeaderMap, Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::any;
use axum::{Router, serve};
use davenda_cache::DistributedCacheBackend;
use davenda_config::{
    DatabaseDriver, DistributedCache, JobBackend, ObjectStoreKind, SecretRef, SessionStore,
};
use tower::ServiceExt;

use super::*;

#[derive(Debug, Error)]
pub enum RuntimeServerError {
    #[error("HTTP request uses unsupported method `{method}`")]
    UnsupportedMethod { method: String },
    #[error("HTTP request did not include a host header")]
    MissingHost,
    #[error("header `{header}` is not valid UTF-8")]
    InvalidHeaderValue { header: &'static str },
    #[error(transparent)]
    Route(#[from] RouteBuildError),
    #[error(transparent)]
    Execution(#[from] RequestExecutionError),
    #[error(transparent)]
    Secret(#[from] SecretResolutionError),
    #[error(transparent)]
    Render(#[from] RuntimeRenderError),
    #[error(transparent)]
    WasmExecution(#[from] LiveWasmExecutionError),
    #[error("live request authorization does not support auth package `{package}`")]
    UnsupportedAuthPackage { package: String },
    #[error("live request authorization failed: {reason}")]
    Authorization { reason: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretResolutionError {
    #[error("secret `{reference}` was not provided to the runtime")]
    MissingSecret { reference: String },
}

pub trait SecretResolver {
    fn resolve(&self, secret: &SecretRef) -> Result<String, SecretResolutionError>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticSecretResolver {
    values: BTreeMap<String, String>,
}

impl StaticSecretResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_secret(
        mut self,
        secret: SecretRef,
        value: impl Into<String>,
    ) -> Result<Self, SecretResolutionError> {
        self.values.insert(secret.redacted(), value.into());
        Ok(self)
    }
}

impl SecretResolver for StaticSecretResolver {
    fn resolve(&self, secret: &SecretRef) -> Result<String, SecretResolutionError> {
        self.values.get(&secret.redacted()).cloned().ok_or_else(|| {
            SecretResolutionError::MissingSecret {
                reference: secret.redacted(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseClientTarget {
    pub driver: DatabaseDriver,
    pub url: Option<String>,
    pub min_connections: u16,
    pub max_connections: u16,
    pub statement_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedCacheClientTarget {
    pub backend: DistributedCacheBackend,
    pub purpose: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsClientTarget {
    pub backend: JobBackend,
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStoreClientTarget {
    pub kind: SessionStoreBackendKind,
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreClientTarget {
    pub kind: ObjectStoreKind,
    pub credential_reference: Option<String>,
    pub local_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedBackendClients {
    pub database: DatabaseClientTarget,
    pub distributed_cache: Option<DistributedCacheClientTarget>,
    pub jobs: JobsClientTarget,
    pub session_store: Option<SessionStoreClientTarget>,
    pub object_store: Option<ObjectStoreClientTarget>,
}

impl SharedBackendClients {
    pub fn from_config<R: SecretResolver>(
        config: &PlatformConfig,
        resolver: &R,
    ) -> Result<Self, SecretResolutionError> {
        let database = DatabaseClientTarget {
            driver: config.database.driver,
            url: config
                .database
                .url
                .as_ref()
                .map(|secret| resolver.resolve(secret))
                .transpose()?,
            min_connections: config.database.min_connections,
            max_connections: config.database.max_connections,
            statement_timeout_secs: config.database.statement_timeout_secs,
        };
        let distributed_cache = config.cache.l2.map(|backend| DistributedCacheClientTarget {
            backend: distributed_cache_backend(backend),
            purpose: "cache-and-coordination",
        });
        let jobs = JobsClientTarget {
            backend: config.jobs.backend,
            shared: true,
        };
        let session_store = match config.http.session.store {
            SessionStore::Memory => None,
            SessionStore::Database => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Database,
                shared: true,
            }),
            SessionStore::Redis => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Redis,
                shared: true,
            }),
            SessionStore::Valkey => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Valkey,
                shared: true,
            }),
        };
        let object_store_credentials = config
            .storage
            .object_store_secret
            .as_ref()
            .map(|secret| resolver.resolve(secret))
            .transpose()?;
        let object_store = config
            .storage
            .object_store
            .map(|kind| ObjectStoreClientTarget {
                kind,
                credential_reference: object_store_credentials.clone(),
                local_root: config.storage.local_root.clone(),
            });

        Ok(Self {
            database,
            distributed_cache,
            jobs,
            session_store,
            object_store,
        })
    }
}

type RouteAuthorizationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, RuntimeServerError>> + Send + 'a>>;

pub(crate) trait LiveRouteCapabilityAuthorizer: Send + Sync {
    fn check_capability<'a>(
        &'a self,
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a>;
}

struct DeferredPostgresRouteCapabilityAuthorizer {
    data: DataRuntimeServices,
    auth_package_name: String,
    authorizer: OnceLock<Result<PostgresRouteCapabilityAuthorizer, String>>,
}

impl DeferredPostgresRouteCapabilityAuthorizer {
    const TENANT_ID: i64 = 1;

    fn new(data: DataRuntimeServices, auth_package_name: String) -> Self {
        Self {
            data,
            auth_package_name,
            authorizer: OnceLock::new(),
        }
    }

    fn authorizer(&self) -> Result<&PostgresRouteCapabilityAuthorizer, RuntimeServerError> {
        match self.authorizer.get_or_init(|| self.build_authorizer()) {
            Ok(authorizer) => Ok(authorizer),
            Err(reason) => Err(RuntimeServerError::Authorization {
                reason: reason.clone(),
            }),
        }
    }

    fn build_authorizer(&self) -> Result<PostgresRouteCapabilityAuthorizer, String> {
        let package =
            default_live_auth_package(&self.auth_package_name).map_err(|error| match error {
                RuntimeServerError::UnsupportedAuthPackage { package } => {
                    format!("unsupported auth package `{package}`")
                }
                other => other.to_string(),
            })?;
        let client = self
            .data
            .connect_lazy_postgres()
            .map_err(|error| error.to_string())?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());

        Ok(PostgresRouteCapabilityAuthorizer {
            auth: davenda_auth::DavendaAuth::new(engine, Self::TENANT_ID),
            package,
        })
    }
}

impl fmt::Debug for DeferredPostgresRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeferredPostgresRouteCapabilityAuthorizer")
            .field("auth_package_name", &self.auth_package_name)
            .finish_non_exhaustive()
    }
}

impl LiveRouteCapabilityAuthorizer for DeferredPostgresRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            self.authorizer()?
                .check_capability(subject, capability, object)
                .await
        })
    }
}

struct PostgresRouteCapabilityAuthorizer {
    auth: davenda_auth::DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: davenda_auth::DefaultAuthModelPackage,
}

impl PostgresRouteCapabilityAuthorizer {
    async fn check_capability(
        &self,
        subject: &davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &davenda_auth::Entity,
    ) -> Result<bool, RuntimeServerError> {
        self.auth
            .check_capability(&self.package, subject, capability, object)
            .await
            .map_err(|error| RuntimeServerError::Authorization {
                reason: error.to_string(),
            })
    }
}

impl fmt::Debug for PostgresRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresRouteCapabilityAuthorizer")
            .field("tenant_id", &self.auth.tenant_id())
            .field("auth_package", &self.package.manifest().name)
            .finish()
    }
}

fn default_live_auth_package(
    auth_package_name: &str,
) -> Result<davenda_auth::DefaultAuthModelPackage, RuntimeServerError> {
    let package = davenda_auth::DefaultAuthModelPackage::default();
    if package.manifest().name == auth_package_name {
        Ok(package)
    } else {
        Err(RuntimeServerError::UnsupportedAuthPackage {
            package: auth_package_name.to_string(),
        })
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAuthorizationCheck {
    pub subject: davenda_auth::DefaultSubject,
    pub capability: davenda_auth::Capability,
    pub object: davenda_auth::Entity,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct StaticLiveRouteCapabilityAuthorizer {
    allowed: Vec<LiveAuthorizationCheck>,
    checks: Arc<Mutex<Vec<LiveAuthorizationCheck>>>,
}

#[cfg(test)]
impl StaticLiveRouteCapabilityAuthorizer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn allowing(
        mut self,
        subject: davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: davenda_auth::Entity,
    ) -> Self {
        self.allowed.push(LiveAuthorizationCheck {
            subject,
            capability,
            object,
        });
        self
    }

    pub(crate) fn checks(&self) -> Vec<LiveAuthorizationCheck> {
        self.checks
            .lock()
            .expect("static live authorizer mutex poisoned")
            .clone()
    }
}

#[cfg(test)]
impl LiveRouteCapabilityAuthorizer for StaticLiveRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            let check = LiveAuthorizationCheck {
                subject: subject.clone(),
                capability,
                object: object.clone(),
            };
            self.checks
                .lock()
                .expect("static live authorizer mutex poisoned")
                .push(check.clone());
            Ok(self.allowed.iter().any(|allowed| allowed == &check))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveHttpRequest {
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
    pub scheme: String,
    pub forwarded_proto: Option<String>,
    pub request_id: Option<String>,
    pub session_cookie: Option<String>,
    pub flash_cookie: Option<String>,
    pub csrf_token: Option<String>,
    pub maintenance_bypass_token: Option<String>,
}

impl LiveHttpRequest {
    pub fn from_request(
        request: &Request<Body>,
        browser: &BrowserSecurityServices,
    ) -> Result<Self, RuntimeServerError> {
        let headers = request.headers();
        let host = header_value(headers, HOST)?.ok_or(RuntimeServerError::MissingHost)?;
        let forwarded_proto = header_value(headers, "x-forwarded-proto")?;
        let scheme = forwarded_proto
            .clone()
            .unwrap_or_else(|| "http".to_string());
        let request_id = header_value(headers, "x-request-id")?;
        let cookies = parse_cookie_header(headers)?;

        Ok(Self {
            method: map_http_method(request.method())?,
            host,
            path: request.uri().path().to_string(),
            scheme,
            forwarded_proto,
            request_id,
            session_cookie: cookies.get(&browser.sessions.session_cookie.name).cloned(),
            flash_cookie: cookies.get(&browser.sessions.flash_cookie.name).cloned(),
            csrf_token: header_value(headers, browser.csrf.header_name.as_str())?,
            maintenance_bypass_token: header_value(headers, "x-davenda-maintenance-bypass")?,
        })
    }

    pub fn into_request_input(self) -> Result<RequestInput, RuntimeServerError> {
        let mut request =
            RequestInput::new(self.method, self.host, self.path)?.with_scheme(self.scheme);

        if let Some(proto) = self.forwarded_proto {
            request = request.with_forwarded_proto(proto);
        }
        if let Some(request_id) = self.request_id {
            request = request.with_request_id(request_id);
        }
        if let Some(session_cookie) = self.session_cookie {
            request = request.with_session_cookie(session_cookie);
        }
        if let Some(flash_cookie) = self.flash_cookie {
            request = request.with_flash_cookie(flash_cookie);
        }
        if let Some(csrf_token) = self.csrf_token {
            request = request.with_csrf_token(csrf_token);
        }
        if let Some(bypass) = self.maintenance_bypass_token {
            request = request.with_maintenance_bypass_token(bypass);
        }

        Ok(request)
    }
}

pub(crate) struct RuntimeServerState {
    plan: RuntimePlan,
    browser: Mutex<BrowserHost>,
    cookie_secret: Vec<u8>,
    csrf_secret: Vec<u8>,
    backends: SharedBackendClients,
    route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
}

impl fmt::Debug for RuntimeServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeServerState")
            .field("plan", &self.plan)
            .field("browser", &self.browser)
            .field("backends", &self.backends)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct HttpServerHost {
    state: Arc<RuntimeServerState>,
    router: Router,
}

impl HttpServerHost {
    pub(crate) fn new(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
    ) -> Self {
        let materializer = RuntimeBackendMaterializer::new(backends.clone());
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.auth_package_name.clone(),
            ));
        let browser = materializer.browser_host(plan.config.app.name.clone(), plan.browser.clone());
        let _shared_cache_runtime = materializer.cache_runtime(plan.cache_planner);
        let _shared_jobs_coordinator = materializer.jobs_coordinator(&plan.jobs);
        Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        )
    }

    pub fn new_with_browser_host(
        plan: RuntimePlan,
        browser: BrowserHost,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
    ) -> Self {
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.auth_package_name.clone(),
            ));
        Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        )
    }

    pub(crate) fn new_with_authorizer(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
    ) -> Self {
        let browser = plan.browser_host();
        Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        )
    }

    fn new_with_browser_and_authorizer(
        plan: RuntimePlan,
        browser: BrowserHost,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
    ) -> Self {
        let state = Arc::new(RuntimeServerState {
            browser: Mutex::new(browser),
            plan,
            cookie_secret,
            csrf_secret,
            backends,
            route_authorizer,
        });
        let router = Router::new()
            .route("/", any(serve_runtime_request))
            .fallback(any(serve_runtime_request))
            .with_state(state.clone());

        Self { state, router }
    }

    pub fn shared_backends(&self) -> &SharedBackendClients {
        &self.state.backends
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn issue_session(
        &self,
        request: SessionIssueRequest,
        now: BrowserInstant,
    ) -> Result<IssuedBrowserSession, RuntimeBrowserError> {
        let mut browser = self
            .state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser.issue_session(request, &self.state.cookie_secret, now)
    }

    pub async fn respond(
        &self,
        request: Request<Body>,
    ) -> Result<Response<Body>, RuntimeServerError> {
        self.router.clone().oneshot(request).await.map_err(|error| {
            RuntimeServerError::UnsupportedMethod {
                method: error.to_string(),
            }
        })
    }

    pub async fn serve(self, listener: tokio::net::TcpListener) -> std::io::Result<()> {
        serve(listener, self.router)
            .await
            .map_err(std::io::Error::other)
    }
}

pub(crate) async fn serve_runtime_request(
    State(state): State<Arc<RuntimeServerState>>,
    request: Request<Body>,
) -> Response<Body> {
    match execute_live_request(&state, request).await {
        Ok(response) => response,
        Err(error) => error_response(error),
    }
}

async fn execute_live_request(
    state: &RuntimeServerState,
    request: Request<Body>,
) -> Result<Response<Body>, RuntimeServerError> {
    let mut request =
        LiveHttpRequest::from_request(&request, &state.plan.browser)?.into_request_input()?;
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let execution = if request.session_cookie.is_some() || request.flash_cookie.is_some() {
        let resolved = {
            let mut browser = state
                .browser
                .lock()
                .expect("runtime browser mutex poisoned");
            browser
                .resolve_request(&request, &state.cookie_secret, now)
                .map_err(RequestExecutionError::from_browser_error)?
        };

        request.session_id = resolved.session.session_id.clone();
        request.session_cookie = None;
        request.flash_cookie = None;

        if request.principal_id.is_none() {
            request.principal_id = resolved.principal_id.clone();
        }

        authorize_live_request(state, &mut request).await?;

        let mut execution =
            state
                .plan
                .execute_request(request, &state.cookie_secret, &state.csrf_secret)?;
        execution.session = resolved.session;
        if execution.principal.principal_id.is_none() {
            execution.principal.principal_id = resolved.principal_id;
        }
        execution.flash_messages = resolved.flash_messages;
        execution.response_cookies = resolved.response_cookies;
        execution
    } else {
        authorize_live_request(state, &mut request).await?;
        state
            .plan
            .execute_request(request, &state.cookie_secret, &state.csrf_secret)?
    };

    execution_response(&state.plan, execution)
}

async fn authorize_live_request(
    state: &RuntimeServerState,
    request: &mut RequestInput,
) -> Result<(), RuntimeServerError> {
    let matched = state
        .plan
        .http
        .resolve_match(
            &state.plan.config,
            request.method,
            &request.host,
            &request.path,
        )
        .ok_or_else(|| {
            RuntimeServerError::Execution(RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })
        })?;

    let RouteAuthGate::Capability(capability) = matched.resolved.auth else {
        return Ok(());
    };
    if request.session_id.is_none() {
        return Ok(());
    }

    let Some(principal_id) = request.principal_id.as_deref() else {
        return Ok(());
    };
    let package = default_live_auth_package(&state.plan.auth_package_name)?;
    let module_manifest = matched.route.module.as_deref().and_then(|module_name| {
        state
            .plan
            .modules
            .iter()
            .find(|manifest| manifest.name == module_name)
    });
    let Some(object) = matched
        .resolved
        .capability_auth_resource(&matched.route, module_manifest, &package)
        .map_err(|error| RuntimeServerError::Authorization {
            reason: error.to_string(),
        })?
    else {
        return Ok(());
    };
    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(principal_id.to_string()));
    let allowed = state
        .route_authorizer
        .check_capability(&subject, capability, &object)
        .await?;

    if allowed {
        request.granted_capabilities.insert(capability);
        Ok(())
    } else {
        Err(RuntimeServerError::Execution(
            RequestExecutionError::CapabilityRequired {
                route: matched.resolved.route_name.clone(),
                capability,
            },
        ))
    }
}

fn execution_response(
    plan: &RuntimePlan,
    execution: RequestExecution,
) -> Result<Response<Body>, RuntimeServerError> {
    let receipts = LiveExecutionReceipts::collect(plan, &execution)?;

    let mut response = match &execution.response {
        HandlerResponse::Page(page) => html_response(
            StatusCode::from_u16(page.status).unwrap_or(StatusCode::OK),
            plan.render_page_response(&execution, &page)?,
        ),
        HandlerResponse::Fragment(fragment) => html_response(
            StatusCode::OK,
            plan.render_fragment_response(&execution, &fragment)?,
        ),
        HandlerResponse::Redirect(redirect) => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() =
                StatusCode::from_u16(redirect.status).unwrap_or(StatusCode::SEE_OTHER);
            response.headers_mut().insert(
                LOCATION,
                redirect
                    .location
                    .parse()
                    .expect("validated redirect location is a header value"),
            );
            response
        }
        HandlerResponse::Json(json) => {
            let mut parts = Vec::new();
            for (key, value) in &json.payload {
                parts.push(format!(
                    "\"{}\":\"{}\"",
                    escape_json(key),
                    escape_json(value)
                ));
            }
            let mut response = text_response(
                StatusCode::from_u16(json.status).unwrap_or(StatusCode::OK),
                format!("{{{}}}", parts.join(",")),
            );
            response.headers_mut().insert(
                "content-type",
                "application/json"
                    .parse()
                    .expect("static content type is valid"),
            );
            response
        }
        HandlerResponse::File(file) => {
            let mut response = Response::new(Body::empty());
            response.headers_mut().insert(
                "content-type",
                file.content_type
                    .parse()
                    .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
            );
            response.headers_mut().insert(
                "x-davenda-file-path",
                file.logical_path
                    .parse()
                    .expect("validated logical path is a header value"),
            );
            response.headers_mut().insert(
                "x-davenda-file-delivery",
                file_delivery_mode_name(file.delivery_mode)
                    .parse()
                    .expect("static delivery mode name is valid"),
            );
            response
        }
    };

    receipts.decorate_response_headers(response.headers_mut());

    response.headers_mut().insert(
        "x-davenda-route",
        execution
            .route
            .route_name
            .parse()
            .expect("validated route name is a header value"),
    );
    response.headers_mut().insert(
        "x-davenda-locale",
        execution
            .locale
            .parse()
            .expect("validated locale is a header value"),
    );
    for (name, value) in execution.cache_plan.headers {
        if let Ok(header_name) = axum::http::HeaderName::try_from(name.as_str()) {
            if let Ok(header_value) = value.parse() {
                response.headers_mut().insert(header_name, header_value);
            }
        }
    }
    for cookie in execution.response_cookies {
        if let Ok(value) = cookie.parse() {
            response.headers_mut().append("set-cookie", value);
        }
    }

    Ok(response)
}

fn error_response(error: RuntimeServerError) -> Response<Body> {
    match error {
        RuntimeServerError::Execution(RequestExecutionError::RouteNotFound { .. }) => {
            (StatusCode::NOT_FOUND, "not found").into_response()
        }
        RuntimeServerError::Execution(RequestExecutionError::SessionRequired { .. }) => {
            (StatusCode::UNAUTHORIZED, "session required").into_response()
        }
        RuntimeServerError::Execution(RequestExecutionError::CapabilityRequired { .. }) => {
            (StatusCode::FORBIDDEN, "capability required").into_response()
        }
        RuntimeServerError::Execution(
            RequestExecutionError::MissingCsrfToken { .. }
            | RequestExecutionError::MissingSessionForCsrf { .. }
            | RequestExecutionError::InvalidCsrfToken { .. },
        ) => (StatusCode::FORBIDDEN, "csrf rejected").into_response(),
        RuntimeServerError::Execution(RequestExecutionError::MaintenanceMode { .. }) => {
            (StatusCode::SERVICE_UNAVAILABLE, "maintenance mode").into_response()
        }
        RuntimeServerError::Execution(RequestExecutionError::FeatureFlagDisabled { .. }) => {
            (StatusCode::NOT_FOUND, "feature disabled").into_response()
        }
        RuntimeServerError::MissingHost | RuntimeServerError::InvalidHeaderValue { .. } => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        RuntimeServerError::Execution(_) => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

fn map_http_method(method: &Method) -> Result<HttpMethod, RuntimeServerError> {
    match *method {
        Method::GET => Ok(HttpMethod::Get),
        Method::HEAD => Ok(HttpMethod::Head),
        Method::POST => Ok(HttpMethod::Post),
        Method::PUT => Ok(HttpMethod::Put),
        Method::PATCH => Ok(HttpMethod::Patch),
        Method::DELETE => Ok(HttpMethod::Delete),
        _ => Err(RuntimeServerError::UnsupportedMethod {
            method: method.to_string(),
        }),
    }
}

fn parse_cookie_header(
    headers: &HeaderMap,
) -> Result<BTreeMap<String, String>, RuntimeServerError> {
    let Some(raw) = headers.get(COOKIE) else {
        return Ok(BTreeMap::new());
    };
    let raw = raw
        .to_str()
        .map_err(|_| RuntimeServerError::InvalidHeaderValue { header: "cookie" })?;
    let mut cookies = BTreeMap::new();
    for segment in raw.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            cookies.insert(name.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(cookies)
}

fn header_value(
    headers: &HeaderMap,
    name: impl AsRef<str>,
) -> Result<Option<String>, RuntimeServerError> {
    let name = name.as_ref();
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    Ok(Some(
        value
            .to_str()
            .map_err(|_| RuntimeServerError::InvalidHeaderValue {
                header: Box::leak(name.to_string().into_boxed_str()),
            })?
            .to_string(),
    ))
}

fn distributed_cache_backend(cache: DistributedCache) -> DistributedCacheBackend {
    match cache {
        DistributedCache::Redis => DistributedCacheBackend::Redis,
        DistributedCache::Valkey => DistributedCacheBackend::Valkey,
    }
}

fn file_delivery_mode_name(mode: FileDeliveryMode) -> &'static str {
    match mode {
        FileDeliveryMode::PublicCdn => "public_cdn",
        FileDeliveryMode::SignedUrl => "signed_url",
        FileDeliveryMode::AppProxy => "app_proxy",
        FileDeliveryMode::LocalOnly => "local_only",
    }
}

fn text_response(status: StatusCode, body: String) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response
}

fn html_response(status: StatusCode, body: String) -> Response<Body> {
    let mut response = text_response(status, body);
    response.headers_mut().insert(
        "content-type",
        "text/html; charset=utf-8"
            .parse()
            .expect("static content type is valid"),
    );
    response
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

impl fmt::Display for SharedBackendClients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "db={:?} cache={:?} jobs={:?} sessions={:?} object_store={:?}",
            self.database.driver,
            self.distributed_cache.as_ref().map(|cache| cache.backend),
            self.jobs.backend,
            self.session_store.as_ref().map(|store| store.kind),
            self.object_store.as_ref().map(|store| store.kind)
        )
    }
}

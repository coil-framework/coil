use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use super::*;
use crate::backends::RuntimeBackendMaterializer;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use axum::routing::{any, get};
use axum::{Router, serve};

mod auth;
mod backend;
mod observability;
mod request;

use auth::DeferredPostgresRouteCapabilityAuthorizer;
pub(crate) use auth::LiveRouteCapabilityAuthorizer;
pub use backend::{
    DatabaseClientTarget, DistributedCacheClientTarget, JobsClientTarget, ObjectStoreClientTarget,
    SecretResolutionError, SecretResolver, SessionStoreClientTarget, SharedBackendClients,
    StaticSecretResolver,
};
pub use request::LiveHttpRequest;
use observability::{
    serve_diagnostics_probe, serve_health_probe, serve_metrics_probe, serve_readiness_probe,
};
use request::{error_response, execute_live_request, serve_runtime_request};

#[cfg(test)]
pub(crate) use auth::{LiveAuthorizationCheck, StaticLiveRouteCapabilityAuthorizer};

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
    #[error(transparent)]
    BrowserHostBuild(#[from] BrowserHostBuildError),
    #[error("request body exceeds configured maximum of {limit} bytes")]
    RequestBodyTooLarge { limit: usize },
    #[error("live request authorization failed: {reason}")]
    Authorization { reason: String },
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
    ) -> Result<Self, RuntimeServerError> {
        let materializer =
            RuntimeBackendMaterializer::new(plan.shared_backend_namespace(), backends.clone());
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.tenant_id(),
                backends.database.url.clone(),
                plan.auth_package.clone(),
            ));
        let browser =
            materializer.browser_host(plan.config.app.name.clone(), plan.browser.clone())?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        ))
    }

    pub fn new_with_browser_host(
        plan: RuntimePlan,
        browser: BrowserHost,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
    ) -> Result<Self, RuntimeServerError> {
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.tenant_id(),
                backends.database.url.clone(),
                plan.auth_package.clone(),
            ));
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        ))
    }

    #[cfg(test)]
    pub(crate) fn new_with_authorizer(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
    ) -> Result<Self, RuntimeServerError> {
        let browser = plan.browser_host()?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
        ))
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
            .route("/health", any(serve_health_probe))
            .route("/ready", any(serve_readiness_probe))
            .route("/metrics", get(serve_metrics_probe))
            .route("/diagnostics", get(serve_diagnostics_probe))
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
        Ok(
            match execute_live_request(&self.state, request, None).await {
                Ok(response) => response,
                Err(error) => error_response(error),
            },
        )
    }

    pub async fn serve(self, listener: tokio::net::TcpListener) -> std::io::Result<()> {
        serve(
            listener,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(std::io::Error::other)
    }
}

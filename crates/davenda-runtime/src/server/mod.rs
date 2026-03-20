use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use super::*;
use crate::backends::RuntimeBackendMaterializer;
use crate::wasm::RuntimeWasmHostServices;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use axum::routing::any;
use axum::{Router, serve};

mod auth;
mod backend;
mod diagnostics;
mod observability;
mod request;

use auth::DeferredPostgresRouteCapabilityAuthorizer;
pub(crate) use auth::LiveRouteCapabilityAuthorizer;
use auth::auth_explain_router;
pub use backend::{
    DatabaseClientTarget, DistributedCacheClientTarget, EnvironmentSecretResolver,
    JobsClientTarget, ObjectStoreClientTarget, SecretResolutionError, SecretResolver,
    SessionStoreClientTarget, SharedBackendClients, StaticSecretResolver,
};
use diagnostics::privileged_router as diagnostics_router;
use observability::public_router as observability_router;
pub use request::LiveHttpRequest;
use request::{error_response, execute_live_request, serve_runtime_request};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use auth::{
    LiveAuthorizationCheck, StaticLiveAuthExplainer, StaticLiveRouteCapabilityAuthorizer,
};

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
    #[error("auth explain failed: {reason}")]
    Explain { reason: String },
}

pub(crate) struct RuntimeServerState {
    plan: RuntimePlan,
    browser: Mutex<BrowserHost>,
    wasm_host: WasmHost,
    cookie_secret: Vec<u8>,
    csrf_secret: Vec<u8>,
    backends: SharedBackendClients,
    route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
    auth_explainer: Option<Arc<dyn auth::LiveAuthExplainer>>,
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
        wasm_secrets: BTreeMap<String, String>,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
    ) -> Result<Self, RuntimeServerError> {
        let materializer = RuntimeBackendMaterializer::new(
            plan.shared_backend_namespace(),
            backends.clone(),
            plan.shared_state_root().clone(),
        );
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.tenant_id(),
                backends.database.url.clone(),
                plan.auth_package.clone(),
            ));
        let auth_explainer = build_auth_explainer(&plan)?;
        let browser =
            materializer.browser_host(plan.config.app.name.clone(), plan.browser.clone())?;
        let storage_host = plan.storage_host_with_object_store(
            backends
                .object_store
                .as_ref()
                .and_then(|backend| backend.object_store_client_config()),
        );
        let wasm_host = WasmHost::with_host_services(
            plan.clone(),
            plan.config.app.name.clone(),
            plan.wasm.clone(),
            plan.extension_registry.clone(),
            plan.config.i18n.default_locale.clone(),
            plan.registered_runtime_jobs.clone(),
            RuntimeWasmHostServices::with_runtime_secrets(plan.clone(), storage_host, wasm_secrets),
        );
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            auth_explainer,
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
        let auth_explainer = build_auth_explainer(&plan)?;
        let wasm_host = plan.wasm_host();
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            auth_explainer,
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
        let wasm_host = plan.wasm_host();
        let auth_explainer = build_auth_explainer(&plan)?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            auth_explainer,
        ))
    }

    #[cfg(test)]
    pub(crate) fn new_with_authorizer_and_explainer(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
        auth_explainer: Arc<dyn auth::LiveAuthExplainer>,
    ) -> Result<Self, RuntimeServerError> {
        let browser = plan.browser_host()?;
        let wasm_host = plan.wasm_host();
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            backends,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            Some(auth_explainer),
        ))
    }

    fn new_with_browser_and_authorizer(
        plan: RuntimePlan,
        browser: BrowserHost,
        wasm_host: WasmHost,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
        auth_explainer: Option<Arc<dyn auth::LiveAuthExplainer>>,
    ) -> Self {
        let state = Arc::new(RuntimeServerState {
            browser: Mutex::new(browser),
            wasm_host,
            plan,
            cookie_secret,
            csrf_secret,
            backends,
            route_authorizer,
            auth_explainer,
        });
        let public_router = observability_router();
        let privileged_router =
            diagnostics_router(state.clone()).merge(auth_explain_router(state.clone()));
        let router = Router::new()
            .merge(public_router)
            .merge(privileged_router)
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

    #[cfg(test)]
    pub(crate) fn public_router(&self) -> Router {
        observability_router().with_state(self.state.clone())
    }

    #[cfg(test)]
    pub(crate) fn privileged_router(&self) -> Router {
        diagnostics_router(self.state.clone())
            .merge(auth_explain_router(self.state.clone()))
            .with_state(self.state.clone())
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

fn build_auth_explainer(
    plan: &RuntimePlan,
) -> Result<Option<Arc<dyn auth::LiveAuthExplainer>>, RuntimeServerError> {
    if !plan.config.auth.explain_api {
        return Ok(None);
    }

    let explainer = davenda_auth::LiveAuthExplainHost::from_runtime(
        &plan.config,
        plan.data.clone(),
        plan.auth_package.clone(),
    )
    .map_err(|error| RuntimeServerError::Explain {
        reason: error.to_string(),
    })?;

    let explainer: Arc<dyn auth::LiveAuthExplainer> = Arc::new(explainer);
    Ok(Some(explainer))
}

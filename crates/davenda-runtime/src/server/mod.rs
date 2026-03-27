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
use davenda_config::{PlatformConfig, SecretRef};

mod auth;
mod backend;
mod devtools;
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
use devtools::development_router;
use diagnostics::privileged_router as diagnostics_router;
use observability::public_router as observability_router;
pub(crate) use request::HostedCheckoutClient;
#[cfg(test)]
pub(crate) type HostedCheckoutSession = request::HostedCheckoutSession;
pub use request::LiveHttpRequest;
use request::{
    LiveStripeHostedCheckoutClient, error_response, execute_live_request, serve_runtime_request,
};

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
    #[error(transparent)]
    Storefront(#[from] StorefrontStateError),
    #[error(transparent)]
    Jobs(#[from] RuntimeJobsError),
    #[error("server configuration is invalid: {reason}")]
    Configuration { reason: String },
    #[error("request body exceeds configured maximum of {limit} bytes")]
    RequestBodyTooLarge { limit: usize },
    #[error("live request authorization failed: {reason}")]
    Authorization { reason: String },
    #[error("auth explain failed: {reason}")]
    Explain { reason: String },
    #[error("linked customer hook rejected `{surface}`: {code}: {message}")]
    CustomerHookRejected {
        surface: &'static str,
        code: String,
        message: String,
    },
    #[error("linked customer hook failed during `{surface}`: {reason}")]
    CustomerHookFailed {
        surface: &'static str,
        reason: String,
    },
}

pub(crate) struct RuntimeServerState {
    plan: RuntimePlan,
    browser: Mutex<BrowserHost>,
    wasm_host: WasmHost,
    storefront: StorefrontStateStore,
    cookie_secret: Vec<u8>,
    csrf_secret: Vec<u8>,
    payment_webhook_secret: Option<String>,
    payment_provider_api_key: Option<String>,
    hosted_checkout_client: Arc<dyn HostedCheckoutClient>,
    backends: SharedBackendClients,
    route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
    auth_explainer: Option<Arc<dyn auth::LiveAuthExplainer>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommercePaymentProviderConfig {
    pub code: String,
    pub checkout_mode: String,
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
        payment_webhook_secret: Option<String>,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
    ) -> Result<Self, RuntimeServerError> {
        let materializer = RuntimeBackendMaterializer::new(
            plan.shared_backend_namespace(),
            backends.clone(),
            plan.shared_state_root().clone(),
        );
        let payment_provider_api_key = configured_commerce_payment_provider(&plan.config)
            .and_then(|provider| provider.api_key_from_runtime_secrets(&wasm_secrets));
        let route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer> =
            Arc::new(DeferredPostgresRouteCapabilityAuthorizer::new(
                plan.data.clone(),
                plan.tenant_id(),
                backends.database.url.clone(),
                plan.auth_package.clone(),
            ));
        let auth_explainer = build_auth_explainer(&plan)?;
        let hosted_checkout_client: Arc<dyn HostedCheckoutClient> =
            Arc::new(LiveStripeHostedCheckoutClient);
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
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            payment_webhook_secret,
            payment_provider_api_key,
            hosted_checkout_client,
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
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        let hosted_checkout_client: Arc<dyn HostedCheckoutClient> =
            Arc::new(LiveStripeHostedCheckoutClient);
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            None,
            None,
            hosted_checkout_client,
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
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        let hosted_checkout_client: Arc<dyn HostedCheckoutClient> =
            Arc::new(LiveStripeHostedCheckoutClient);
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            None,
            None,
            hosted_checkout_client,
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
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        let hosted_checkout_client: Arc<dyn HostedCheckoutClient> =
            Arc::new(LiveStripeHostedCheckoutClient);
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            None,
            None,
            hosted_checkout_client,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            Some(auth_explainer),
        ))
    }

    #[cfg(test)]
    pub(crate) fn new_with_checkout_client(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        wasm_secrets: BTreeMap<String, String>,
        payment_webhook_secret: Option<String>,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        hosted_checkout_client: Arc<dyn HostedCheckoutClient>,
    ) -> Result<Self, RuntimeServerError> {
        let materializer = RuntimeBackendMaterializer::new(
            plan.shared_backend_namespace(),
            backends.clone(),
            plan.shared_state_root().clone(),
        );
        let payment_provider_api_key = configured_commerce_payment_provider(&plan.config)
            .and_then(|provider| provider.api_key_from_runtime_secrets(&wasm_secrets));
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
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            payment_webhook_secret,
            payment_provider_api_key,
            hosted_checkout_client,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            auth_explainer,
        ))
    }

    #[cfg(test)]
    pub(crate) fn new_with_authorizer_and_checkout_client(
        plan: RuntimePlan,
        backends: SharedBackendClients,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
        hosted_checkout_client: Arc<dyn HostedCheckoutClient>,
    ) -> Result<Self, RuntimeServerError> {
        let browser = plan.browser_host()?;
        let wasm_host = plan.wasm_host();
        let auth_explainer = build_auth_explainer(&plan)?;
        let storefront = StorefrontStateStore::open_for_plan(&plan)?;
        Ok(Self::new_with_browser_and_authorizer(
            plan,
            browser,
            wasm_host,
            storefront,
            backends,
            None,
            None,
            hosted_checkout_client,
            cookie_secret,
            csrf_secret,
            route_authorizer,
            auth_explainer,
        ))
    }

    fn new_with_browser_and_authorizer(
        plan: RuntimePlan,
        browser: BrowserHost,
        wasm_host: WasmHost,
        storefront: StorefrontStateStore,
        backends: SharedBackendClients,
        payment_webhook_secret: Option<String>,
        payment_provider_api_key: Option<String>,
        hosted_checkout_client: Arc<dyn HostedCheckoutClient>,
        cookie_secret: Vec<u8>,
        csrf_secret: Vec<u8>,
        route_authorizer: Arc<dyn LiveRouteCapabilityAuthorizer>,
        auth_explainer: Option<Arc<dyn auth::LiveAuthExplainer>>,
    ) -> Self {
        let state = Arc::new(RuntimeServerState {
            browser: Mutex::new(browser),
            wasm_host,
            storefront,
            plan,
            cookie_secret,
            csrf_secret,
            payment_webhook_secret,
            payment_provider_api_key,
            hosted_checkout_client,
            backends,
            route_authorizer,
            auth_explainer,
        });
        let public_router = observability_router();
        let privileged_router =
            diagnostics_router(state.clone()).merge(auth_explain_router(state.clone()));
        let mut router = Router::new()
            .merge(public_router)
            .merge(privileged_router)
            .route("/", any(serve_runtime_request))
            .fallback(any(serve_runtime_request));
        if state.is_development() {
            router = router.merge(development_router());
        }
        let router = router.with_state(state.clone());

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

    #[cfg(test)]
    pub(crate) fn wasm_host(&self) -> WasmHost {
        self.state.wasm_host.clone()
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

impl RuntimeServerState {
    pub(crate) fn is_development(&self) -> bool {
        matches!(
            self.plan.config.app.environment,
            davenda_config::Environment::Development
        )
    }

    pub(crate) fn uses_development_hosted_checkout_stub(&self) -> bool {
        self.is_development()
            && self
                .payment_provider_api_key
                .as_deref()
                .is_none_or(is_placeholder_stripe_secret_key)
    }
}

fn is_placeholder_stripe_secret_key(api_key: &str) -> bool {
    let normalized = api_key.trim();
    normalized.is_empty()
        || matches!(
            normalized,
            "sk_test_replace_me" | "replace-me" | "changeme" | "change-me"
        )
        || normalized.contains("replace_me")
}

pub(crate) fn resolve_commerce_payment_webhook_secret<R: SecretResolver>(
    config: &PlatformConfig,
    resolver: &R,
) -> Result<Option<String>, RuntimeServerError> {
    if let Some(provider) = configured_commerce_payment_provider(config) {
        if let Some(secret) =
            resolve_module_payment_webhook_secret(config, &provider.module_config_key(), resolver)?
        {
            return Ok(Some(secret));
        }
    }

    resolve_module_payment_webhook_secret(config, "commerce", resolver)
}

pub(crate) fn configured_commerce_payment_provider(
    config: &PlatformConfig,
) -> Option<CommercePaymentProviderConfig> {
    let Some(stripe_settings) = config.modules.settings.get("commerce-payments-stripe") else {
        return None;
    };
    let table = stripe_settings.as_table()?;
    let code = table
        .get("provider")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("stripe")
        .to_ascii_lowercase();
    let checkout_mode = table
        .get("checkout_mode")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("webhook-confirmation")
        .to_ascii_lowercase();
    Some(CommercePaymentProviderConfig {
        code,
        checkout_mode,
    })
}

fn resolve_module_payment_webhook_secret<R: SecretResolver>(
    config: &PlatformConfig,
    module_key: &str,
    resolver: &R,
) -> Result<Option<String>, RuntimeServerError> {
    let Some(module_settings) = config.modules.settings.get(module_key) else {
        return Ok(None);
    };
    let Some(table) = module_settings.as_table() else {
        return Err(RuntimeServerError::Configuration {
            reason: format!("modules.{module_key} must be a table when provided"),
        });
    };
    let secret_field = if table.contains_key("webhook_secret") {
        "webhook_secret"
    } else if table.contains_key("payment_webhook_secret") {
        "payment_webhook_secret"
    } else {
        return Ok(None);
    };
    let Some(secret_value) = table.get(secret_field) else {
        return Ok(None);
    };
    let secret_ref: SecretRef =
        secret_value
            .clone()
            .try_into()
            .map_err(|error| RuntimeServerError::Configuration {
                reason: format!("modules.{module_key}.{secret_field} is invalid: {error}"),
            })?;
    Ok(Some(resolver.resolve(&secret_ref)?))
}

impl CommercePaymentProviderConfig {
    pub(crate) fn uses_hosted_checkout(&self) -> bool {
        matches!(
            self.checkout_mode.as_str(),
            "hosted-checkout" | "hosted_checkout" | "stripe-hosted-checkout"
        )
    }

    pub(crate) fn api_key_from_runtime_secrets(
        &self,
        runtime_secrets: &BTreeMap<String, String>,
    ) -> Option<String> {
        self.api_key_secret_candidates()
            .iter()
            .find_map(|candidate| runtime_secrets.get(*candidate))
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    pub(crate) fn module_config_key(&self) -> String {
        match self.code.as_str() {
            "stripe" => "commerce-payments-stripe".to_string(),
            other => format!("commerce-payments-{other}"),
        }
    }

    pub(crate) fn label(&self) -> String {
        match (self.code.as_str(), self.checkout_mode.as_str()) {
            ("stripe", _) if self.uses_hosted_checkout() => "Stripe hosted checkout".to_string(),
            ("stripe", "webhook-confirmation") => "Stripe webhook confirmation".to_string(),
            ("stripe", _) => "Stripe payment provider".to_string(),
            (other, _) if self.uses_hosted_checkout() => {
                format!("{} hosted checkout", display_payment_provider_name(other))
            }
            (other, "webhook-confirmation") => {
                format!(
                    "{} webhook confirmation",
                    display_payment_provider_name(other)
                )
            }
            (other, _) => format!("{} payment provider", display_payment_provider_name(other)),
        }
    }

    pub(crate) fn summary(&self) -> String {
        match (self.code.as_str(), self.checkout_mode.as_str()) {
            ("stripe", _) if self.uses_hosted_checkout() => "This checkout reserves the order in Davenda, then redirects the customer to Stripe Checkout for payment collection. Davenda still waits for the signed Stripe webhook before treating the order as paid.".to_string(),
            ("stripe", "webhook-confirmation") => "This checkout uses the installed Stripe payment provider. The order stays visible until the signed Stripe webhook confirms the payment result.".to_string(),
            ("stripe", _) => "This checkout uses the installed Stripe payment provider for payment confirmation.".to_string(),
            (other, _) if self.uses_hosted_checkout() => format!(
                "This checkout reserves the order in Davenda, then redirects the customer to the installed {} hosted checkout. Davenda still waits for the signed webhook before treating the order as paid.",
                display_payment_provider_name(other)
            ),
            (other, "webhook-confirmation") => format!(
                "This checkout uses the installed {} payment provider. The order stays visible until its signed webhook confirms the payment result.",
                display_payment_provider_name(other)
            ),
            (other, _) => format!(
                "This checkout uses the installed {} payment provider for payment confirmation.",
                display_payment_provider_name(other)
            ),
        }
    }

    pub(crate) fn submit_label(&self) -> String {
        match (self.code.as_str(), self.checkout_mode.as_str()) {
            ("stripe", _) if self.uses_hosted_checkout() => "Continue to Stripe".to_string(),
            ("stripe", "webhook-confirmation") => {
                "Place order and wait for Stripe confirmation".to_string()
            }
            (other, _) if self.uses_hosted_checkout() => {
                format!("Continue to {}", display_payment_provider_name(other))
            }
            (_, "webhook-confirmation") => {
                format!(
                    "Place order and wait for {} confirmation",
                    display_payment_provider_name(&self.code)
                )
            }
            _ => "Place order".to_string(),
        }
    }

    pub(crate) fn pending_confirmation_summary(&self, order_id: &str) -> String {
        match (self.code.as_str(), self.checkout_mode.as_str()) {
            ("stripe", _) if self.uses_hosted_checkout() => format!(
                "Order {order_id} was reserved and handed off to Stripe Checkout. Stripe still needs to confirm payment."
            ),
            ("stripe", "webhook-confirmation") => {
                format!("Order {order_id} was received. Stripe still needs to confirm payment.")
            }
            (_, _) if self.uses_hosted_checkout() => format!(
                "Order {order_id} was reserved and handed off to {}. The provider still needs to confirm payment.",
                display_payment_provider_name(&self.code)
            ),
            (_, "webhook-confirmation") => format!(
                "Order {order_id} was received. {} still needs to confirm payment.",
                display_payment_provider_name(&self.code)
            ),
            _ => format!("Order {order_id} was received and is awaiting payment confirmation."),
        }
    }

    pub(crate) fn pending_next_step(&self) -> String {
        match (self.code.as_str(), self.checkout_mode.as_str()) {
            ("stripe", _) if self.uses_hosted_checkout() => {
                "Stripe Checkout has not confirmed this payment yet. The order will move forward after the hosted Stripe session finishes and the signed Stripe webhook arrives.".to_string()
            }
            ("stripe", "webhook-confirmation") => {
                "Stripe has not confirmed this payment yet. The order will move forward after the signed Stripe webhook arrives.".to_string()
            }
            (_, _) if self.uses_hosted_checkout() => format!(
                "{} has not confirmed this payment yet. The order will move forward after the hosted checkout finishes and the provider webhook arrives.",
                display_payment_provider_name(&self.code)
            ),
            (_, "webhook-confirmation") => format!(
                "{} has not confirmed this payment yet. The order will move forward after its signed webhook arrives.",
                display_payment_provider_name(&self.code)
            ),
            _ => "Payment confirmation is pending. The order will move forward after the provider callback arrives.".to_string(),
        }
    }

    fn api_key_secret_candidates(&self) -> &'static [&'static str] {
        match self.code.as_str() {
            "stripe" => &[
                "commerce_payments_stripe_secret_key",
                "commerce_payments_stripe_api_key",
                "stripe_secret_key",
                "stripe_api_key",
            ],
            _ => &[],
        }
    }
}

fn display_payment_provider_name(code: &str) -> String {
    match code {
        "stripe" => "Stripe".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => "Payment provider".to_string(),
            }
        }
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

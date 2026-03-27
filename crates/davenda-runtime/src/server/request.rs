use super::auth::authorize_live_request;
use super::*;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HOST};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use davenda_customer_sdk::{
    AuditEntry, AuditFacade, AuthCheckRequest, AuthCheckResult, AuthExplainRequest,
    AuthExplanation, AuthFacade, BackendError, BackendErrorKind, CmsPageDraft, CmsPublishDecision,
    CommerceFacade, CommerceProduct, CustomerAppContext as SdkCustomerAppContext, Headers,
    JobReceipt, JobsFacade, MoneyAmount, OrderDraft, OrderLineDraft, OrderReviewDecision,
    OutboundHttpFacade, OutboundHttpRequest, OutboundHttpResponse,
    PrincipalContext as SdkPrincipalContext, PrincipalKind as SdkPrincipalKind, RepositoryFacade,
    RepositoryQuery, RepositoryRecord, RepositoryRecordSet, RepositoryWrite,
    RepositoryWriteReceipt, RequestContext as SdkRequestContext, TraceContext as SdkTraceContext,
    VerifiedWebhook, WebhookHandlingResult,
};
use davenda_jobs::JobInstant;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

const STOREFRONT_ORDER_HISTORY_JSON_PATH: &str = "/account/orders.json";
const STOREFRONT_FORM_CSRF_HEADERS: &[(&str, &str)] = &[
    (
        "/cart/items",
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    ),
    ("/cart", "x-davenda-storefront-csrf-commerce-cart-update"),
    (
        "/checkout/start",
        "x-davenda-storefront-csrf-commerce-checkout-start",
    ),
    (
        "/checkout/complete",
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    ),
    (
        "/admin/catalog/products",
        "x-davenda-storefront-csrf-commerce-catalog-admin-update",
    ),
    (
        "/admin/orders/refund",
        "x-davenda-storefront-csrf-commerce-order-refund",
    ),
    (
        "/admin/orders/fulfill",
        "x-davenda-storefront-csrf-commerce-order-fulfill",
    ),
];
const CMS_ADMIN_FORM_CSRF_HEADERS: &[(&str, &str)] = &[
    (
        "/admin/pages/draft",
        "x-davenda-cms-csrf-cms-pages-save-draft",
    ),
    (
        "/admin/pages/publish",
        "x-davenda-cms-csrf-cms-pages-publish",
    ),
    (
        "/admin/pages/unpublish",
        "x-davenda-cms-csrf-cms-pages-unpublish",
    ),
    (
        "/admin/navigation/save",
        "x-davenda-cms-csrf-cms-navigation-save",
    ),
    (
        "/admin/redirects/save",
        "x-davenda-cms-csrf-cms-redirects-save",
    ),
];
const STOREFRONT_NATIVE_CAPABILITY_ROUTES: &[&str] = &[
    "commerce.cart",
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout",
    "commerce.checkout-start",
    "commerce.checkout-complete",
    "commerce.checkout-confirmation",
    "commerce.catalog-admin-update",
    "commerce.account-session-end",
    "commerce.order-refund",
    "commerce.order-fulfill",
];
const CMS_ADMIN_NATIVE_MUTATION_ROUTES: &[&str] = &[
    "cms.pages.save-draft",
    "cms.pages.publish",
    "cms.pages.unpublish",
    "cms.navigation.save",
    "cms.redirects.save",
];
const STOREFRONT_CSRF_ACTIONS: &[&str] = &[
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout-start",
    "commerce.checkout-complete",
    "commerce.catalog-admin-update",
    "commerce.account-session-end",
    "commerce.order-refund",
    "commerce.order-fulfill",
];
const CMS_ADMIN_CSRF_ACTIONS: &[(&str, &str)] = &[
    (
        "cms.pages.save-draft",
        "x-davenda-cms-csrf-cms-pages-save-draft",
    ),
    ("cms.pages.publish", "x-davenda-cms-csrf-cms-pages-publish"),
    (
        "cms.pages.unpublish",
        "x-davenda-cms-csrf-cms-pages-unpublish",
    ),
    (
        "cms.navigation.save",
        "x-davenda-cms-csrf-cms-navigation-save",
    ),
    (
        "cms.redirects.save",
        "x-davenda-cms-csrf-cms-redirects-save",
    ),
];

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedPaymentWebhook {
    event: String,
    payment_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CatalogAdminMutationInput {
    Product(crate::storefront::StorefrontCatalogProductUpdate),
    Collection(crate::storefront::StorefrontCatalogCollectionUpdate),
}

#[derive(Debug, Deserialize)]
struct StripeCheckoutSessionResponse {
    id: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostedCheckoutSession {
    pub(crate) id: String,
    pub(crate) url: String,
}

pub(crate) trait HostedCheckoutClient: Send + Sync {
    fn create_stripe_checkout_session(
        &self,
        api_key: &str,
        request_body: &str,
        idempotency_key: &str,
    ) -> Result<HostedCheckoutSession, String>;
}

#[derive(Debug, Default)]
pub(crate) struct LiveStripeHostedCheckoutClient;

impl HostedCheckoutClient for LiveStripeHostedCheckoutClient {
    fn create_stripe_checkout_session(
        &self,
        api_key: &str,
        request_body: &str,
        idempotency_key: &str,
    ) -> Result<HostedCheckoutSession, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout_read(std::time::Duration::from_secs(10))
            .build();
        let request = agent
            .post("https://api.stripe.com/v1/checkout/sessions")
            .set("authorization", &format!("Bearer {api_key}"))
            .set("content-type", "application/x-www-form-urlencoded")
            .set("idempotency-key", idempotency_key);
        match request.send_string(request_body) {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|error| format!("failed to read Stripe Checkout response: {error}"))?;
                let session = serde_json::from_str::<StripeCheckoutSessionResponse>(&body)
                    .map_err(|error| {
                        format!("failed to decode Stripe Checkout response: {error}")
                    })?;
                Ok(HostedCheckoutSession {
                    id: session.id,
                    url: session.url,
                })
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                Err(format!(
                    "Stripe Checkout session creation failed with HTTP {code}: {body}"
                ))
            }
            Err(ureq::Error::Transport(error)) => {
                Err(format!("Stripe Checkout handoff request failed: {error}"))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveHttpRequest {
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
    pub query_params: RequestFieldMap,
    pub form_fields: RequestFieldMap,
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
        server: &davenda_config::ServerConfig,
        remote_addr: Option<SocketAddr>,
    ) -> Result<Self, RuntimeServerError> {
        let headers = request.headers();
        let host = header_value(headers, HOST)?.ok_or(RuntimeServerError::MissingHost)?;
        let trusted_forwarded_headers = server.trusts_forwarded_headers(remote_addr.as_ref());
        let forwarded_proto = if trusted_forwarded_headers {
            header_value(headers, "x-forwarded-proto")?
        } else {
            None
        };
        let scheme = forwarded_proto
            .clone()
            .unwrap_or_else(|| "http".to_string());
        let request_id = header_value(headers, "x-request-id")?;
        let cookies = parse_cookie_header(headers)?;

        Ok(Self {
            method: map_http_method(request.method())?,
            host,
            path: request.uri().path().to_string(),
            query_params: parse_request_fields(
                request.uri().query().unwrap_or_default().as_bytes(),
            ),
            form_fields: RequestFieldMap::new(),
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
        let mut request = RequestInput::new(self.method, self.host, self.path)?
            .with_query_params(self.query_params)
            .with_form_fields(self.form_fields)
            .with_scheme(self.scheme);

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

pub(crate) async fn serve_runtime_request(
    State(state): State<Arc<RuntimeServerState>>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    match execute_live_request(&state, request, Some(remote_addr)).await {
        Ok(response) => response,
        Err(error) => error_response(error),
    }
}

pub(super) async fn execute_live_request(
    state: &RuntimeServerState,
    request: Request<Body>,
    remote_addr: Option<SocketAddr>,
) -> Result<Response<Body>, RuntimeServerError> {
    let raw_request =
        enforce_request_body_limit(request, state.plan.config.server.max_body_bytes).await?;
    let mut live_request = LiveHttpRequest::from_request(
        &raw_request,
        &state.plan.browser,
        &state.plan.config.server,
        remote_addr,
    )?;
    live_request.form_fields = parse_form_fields(live_request.method, raw_request).await?;
    let mut request = live_request.into_request_input()?;
    if request.method == HttpMethod::Get
        && state
            .plan
            .http
            .resolve_match(
                &state.plan.config,
                request.method,
                &request.host,
                &request.path,
            )
            .is_none()
    {
        if let Some(response) = cms_admin_redirect_response(state, &request.path)? {
            return Ok(response);
        }
    }
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let mut native_response_cookies = Vec::new();
    let mut execution = if request.session_cookie.is_some() || request.flash_cookie.is_some() {
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

        native_response_cookies.extend(resolved.response_cookies.clone());
        prepare_native_storefront_request(state, &mut request, now, &mut native_response_cookies)?;
        if request.path == STOREFRONT_ORDER_HISTORY_JSON_PATH && request.method == HttpMethod::Get {
            return storefront_order_history_response(state, &request, native_response_cookies);
        }
        authorize_live_request(state, &mut request).await?;
        let resolved_session = SessionContext {
            session_id: request.session_id.clone(),
            resolved_from_cookie: resolved.session.resolved_from_cookie,
        };

        let mut execution =
            state
                .plan
                .execute_request(request, &state.cookie_secret, &state.csrf_secret)?;
        execution.session = resolved_session;
        if execution.principal.principal_id.is_none() {
            execution.principal.principal_id = resolved.principal_id;
        }
        execution.flash_messages = resolved.flash_messages;
        execution.response_cookies = native_response_cookies.clone();
        execution
    } else {
        prepare_native_storefront_request(state, &mut request, now, &mut native_response_cookies)?;
        if request.path == STOREFRONT_ORDER_HISTORY_JSON_PATH && request.method == HttpMethod::Get {
            return storefront_order_history_response(state, &request, native_response_cookies);
        }
        authorize_live_request(state, &mut request).await?;
        let mut execution =
            state
                .plan
                .execute_request(request, &state.cookie_secret, &state.csrf_secret)?;
        execution.response_cookies = native_response_cookies;
        execution
    };

    let mut storefront_mutation_cookies = Vec::new();
    if let Some(location) =
        apply_native_cms_admin_mutations(state, &execution, now, &mut storefront_mutation_cookies)?
    {
        execution
            .response_cookies
            .extend(storefront_mutation_cookies);
        return Ok(storefront_redirect_response(
            &location,
            &execution.response_cookies,
        ));
    }
    if let Some(location) =
        apply_native_storefront_mutations(state, &execution, now, &mut storefront_mutation_cookies)
            .await?
    {
        execution
            .response_cookies
            .extend(storefront_mutation_cookies);
        return Ok(storefront_redirect_response(
            &location,
            &execution.response_cookies,
        ));
    }
    execution
        .response_cookies
        .extend(storefront_mutation_cookies);
    let route_name = execution.route.route_name.clone();
    let method = execution.method;
    let session_id = execution.session.session_id.clone();
    let principal_id = execution.principal.principal_id.clone();
    let provider_result = execution_query_field(&execution, "provider_result").map(str::to_string);
    let payment_reference =
        execution_query_field(&execution, "payment_reference").map(str::to_string);
    if let Some(location) = redirect_failed_checkout_confirmation(
        state,
        route_name.as_str(),
        method,
        session_id.as_deref(),
        principal_id.as_deref(),
        provider_result.as_deref(),
        payment_reference.as_deref(),
        now,
        &mut execution.response_cookies,
    )? {
        return Ok(storefront_redirect_response(
            &location,
            &execution.response_cookies,
        ));
    }
    let augmentation = storefront_response_augmentation(state, &execution)?;
    let response = execution_response(&state.plan, &state.wasm_host, execution)?;
    apply_storefront_response_augmentation(response, augmentation).await
}

fn execution_response(
    plan: &RuntimePlan,
    wasm_host: &WasmHost,
    execution: RequestExecution,
) -> Result<Response<Body>, RuntimeServerError> {
    let receipts = LiveExecutionReceipts::collect(plan, wasm_host, &execution)?;
    Ok(receipts.compose_response(plan, &execution)?.into_response())
}

pub(super) fn error_response(error: RuntimeServerError) -> Response<Body> {
    match error {
        RuntimeServerError::Storefront(
            StorefrontStateError::UnknownSku { .. }
            | StorefrontStateError::InvalidQuantity
            | StorefrontStateError::MissingPaymentMethod
            | StorefrontStateError::MissingCheckoutEmail
            | StorefrontStateError::InvalidPaymentLast4
            | StorefrontStateError::MissingPaymentIntent
            | StorefrontStateError::PaymentIntentMismatch { .. }
            | StorefrontStateError::CheckoutNotReady { .. }
            | StorefrontStateError::EmptyCart { .. }
            | StorefrontStateError::UnknownPaymentReference { .. }
            | StorefrontStateError::UnknownPaymentWebhookEvent { .. }
            | StorefrontStateError::UnexpectedPaymentWebhookProvider { .. }
            | StorefrontStateError::InvalidPaymentWebhookSignature,
        ) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        RuntimeServerError::Storefront(StorefrontStateError::MissingPaymentWebhookSecret) => {
            (StatusCode::SERVICE_UNAVAILABLE, error.to_string()).into_response()
        }
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
        RuntimeServerError::RequestBodyTooLarge { .. } => {
            (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response()
        }
        RuntimeServerError::CustomerHookRejected { .. } => {
            (StatusCode::CONFLICT, error.to_string()).into_response()
        }
        RuntimeServerError::CustomerHookFailed { .. } => {
            (StatusCode::SERVICE_UNAVAILABLE, error.to_string()).into_response()
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

async fn parse_form_fields(
    request_method: HttpMethod,
    request: Request<Body>,
) -> Result<RequestFieldMap, RuntimeServerError> {
    if !matches!(
        request_method,
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete
    ) {
        return Ok(RequestFieldMap::new());
    }

    let is_form = request
        .headers()
        .get(CONTENT_TYPE)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| RuntimeServerError::InvalidHeaderValue {
                    header: "content-type",
                })
                .map(|content_type| {
                    content_type
                        .split(';')
                        .next()
                        .map(str::trim)
                        .is_some_and(|mime| {
                            mime.eq_ignore_ascii_case("application/x-www-form-urlencoded")
                        })
                })
        })
        .transpose()?
        .unwrap_or(false);

    if !is_form {
        return Ok(RequestFieldMap::new());
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| RuntimeServerError::RequestBodyTooLarge { limit: usize::MAX })?;
    Ok(parse_request_fields(&bytes))
}

fn parse_request_fields(bytes: &[u8]) -> RequestFieldMap {
    let mut fields = RequestFieldMap::new();
    for (name, value) in form_urlencoded::parse(bytes) {
        push_request_field(&mut fields, name.into_owned(), value.into_owned());
    }
    fields
}

fn prepare_native_storefront_request(
    state: &RuntimeServerState,
    request: &mut RequestInput,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<(), RuntimeServerError> {
    let Some(matched) = state.plan.http.resolve_match(
        &state.plan.config,
        request.method,
        &request.host,
        &request.path,
    ) else {
        return Ok(());
    };

    let route_name = matched.resolved.route_name.as_str();
    let is_storefront_page = request.method == HttpMethod::Get
        && matched.route.module.as_deref() == Some("commerce")
        && matched.route.area != RouteArea::Admin
        && matched.route.area != RouteArea::Api
        && matched.route.area != RouteArea::Fragment;
    let is_account_page =
        request.method == HttpMethod::Get && matched.route.area == RouteArea::Account;
    let is_native_capability_route = STOREFRONT_NATIVE_CAPABILITY_ROUTES.contains(&route_name);
    let is_native_mutation_route = matches!(
        route_name,
        "commerce.add-to-cart"
            | "commerce.cart-update"
            | "commerce.checkout-start"
            | "commerce.checkout-complete"
    );
    if !is_storefront_page && !is_account_page && !is_native_mutation_route {
        return Ok(());
    }

    ensure_storefront_session(state, request, now, response_cookies)?;
    if is_native_capability_route {
        request
            .granted_capabilities
            .insert(davenda_auth::Capability::CheckoutSessionCreate);
    }
    Ok(())
}

fn ensure_storefront_session(
    state: &RuntimeServerState,
    request: &mut RequestInput,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<String, RuntimeServerError> {
    if let Some(session_id) = request.session_id.clone() {
        return Ok(session_id);
    }

    let issued = {
        let mut browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser
            .issue_session(SessionIssueRequest::new(), &state.cookie_secret, now)
            .map_err(RequestExecutionError::from_browser_error)?
    };
    request.session_id = Some(issued.record.session_id.clone());
    response_cookies.push(issued.set_cookie_header);
    Ok(issued.record.session_id)
}

fn push_storefront_flash(
    state: &RuntimeServerState,
    response_cookies: &mut Vec<String>,
    level: FlashLevel,
    text: impl Into<String>,
) -> Result<(), RuntimeServerError> {
    let message =
        FlashMessage::new(level, text.into()).map_err(RequestExecutionError::from_browser_error)?;
    let cookie = {
        let browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser
            .issue_flash_cookie(&state.cookie_secret, &[message])
            .map_err(RequestExecutionError::from_browser_error)?
    };
    response_cookies.push(cookie);
    Ok(())
}

fn push_storefront_form_state(
    state: &RuntimeServerState,
    response_cookies: &mut Vec<String>,
    form_state: &StorefrontFormState,
) -> Result<(), RuntimeServerError> {
    let message = FlashMessage::new(FlashLevel::Error, form_state.encode()?)
        .map_err(RequestExecutionError::from_browser_error)?;
    let cookie = {
        let browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser
            .issue_flash_cookie(&state.cookie_secret, &[message])
            .map_err(RequestExecutionError::from_browser_error)?
    };
    response_cookies.push(cookie);
    Ok(())
}

fn storefront_redirect_response(location: &str, response_cookies: &[String]) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response.headers_mut().insert(
        HeaderName::from_static("location"),
        HeaderValue::from_str(location).expect("redirect location is a valid header value"),
    );
    for cookie in response_cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            response
                .headers_mut()
                .append(HeaderName::from_static("set-cookie"), value);
        }
    }
    response
}

fn cms_admin_redirect_response(
    state: &RuntimeServerState,
    path: &str,
) -> Result<Option<Response<Body>>, RuntimeServerError> {
    if path.starts_with("/admin") {
        return Ok(None);
    }
    let workspace = CmsAdminWorkspace::load(&state.plan).map_err(|reason| {
        RuntimeServerError::Configuration {
            reason: format!("failed to load CMS admin workspace: {reason}"),
        }
    })?;
    let Some(redirect) = workspace.redirect_for_path(path) else {
        return Ok(None);
    };
    let mut response = Response::new(Body::empty());
    *response.status_mut() = if redirect.permanent {
        StatusCode::PERMANENT_REDIRECT
    } else {
        StatusCode::TEMPORARY_REDIRECT
    };
    response.headers_mut().insert(
        HeaderName::from_static("location"),
        HeaderValue::from_str(&redirect.to).expect("redirect target is a valid header value"),
    );
    Ok(Some(response))
}

fn revoke_storefront_session(
    state: &RuntimeServerState,
    session_id: &str,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<(), RuntimeServerError> {
    let clear_cookie = {
        let mut browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        match browser.revoke_session(session_id, now) {
            Ok(()) => {}
            Err(
                RuntimeBrowserError::UnknownSession { .. }
                | RuntimeBrowserError::ExpiredSession { .. }
                | RuntimeBrowserError::RevokedSession { .. },
            ) => {}
            Err(error) => return Err(RequestExecutionError::from_browser_error(error).into()),
        }
        browser.clear_session_cookie_header()
    };
    response_cookies.push(clear_cookie);
    Ok(())
}

fn parse_quantity_field(value: Option<&str>) -> Option<u32> {
    value.and_then(|raw| raw.trim().parse::<u32>().ok())
}

fn storefront_quantity_from_execution(execution: &RequestExecution) -> u32 {
    parse_quantity_field(execution_form_field(execution, "quantity")).unwrap_or(1)
}

fn storefront_form_field_value(execution: &RequestExecution, name: &str) -> String {
    execution_form_field(execution, name)
        .unwrap_or_default()
        .to_string()
}

fn storefront_checkout_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("commerce.checkout", summary.into());
    for field in [
        "checkout_email",
        "delivery_name",
        "delivery_note",
        "payment_method",
        "payment_last4",
        "checkout_intent",
    ] {
        let value = storefront_form_field_value(execution, field);
        if !value.is_empty() {
            state = state.with_field_value(field, value);
        }
    }
    if execution_form_field(execution, "terms_accepted").is_some() {
        state = state.with_field_value("terms_accepted", "yes");
    }
    state
}

#[derive(Debug)]
struct RuntimeCheckoutCommerceFacade<'a> {
    catalog: &'a StorefrontCatalog,
}

impl CommerceFacade for RuntimeCheckoutCommerceFacade<'_> {
    fn product(&self, sku: &str) -> Result<Option<CommerceProduct>, BackendError> {
        Ok(self
            .catalog
            .product_by_sku_or_handle(sku)
            .map(|product| CommerceProduct {
                sku: product.sku.clone(),
                handle: product.handle.clone(),
                title: product.title.clone(),
                current_price: MoneyAmount::new(product.currency.clone(), product.price_minor),
                collection_handle: Some(product.collection_handle.clone()),
                metadata: BTreeMap::new(),
            }))
    }

    fn add_order_note(&self, _order_id: &str, _note: &str) -> Result<(), BackendError> {
        Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "commerce.order_note.unsupported",
            "Runtime order-note persistence is not implemented for customer checkout hooks yet.",
        ))
    }
}

#[derive(Debug)]
struct RuntimeCheckoutAuthFacade<'a> {
    state: &'a RuntimeServerState,
    execution: &'a RequestExecution,
}

impl AuthFacade for RuntimeCheckoutAuthFacade<'_> {
    fn check_capability(
        &self,
        request: &AuthCheckRequest,
    ) -> Result<AuthCheckResult, BackendError> {
        let capability = parse_customer_capability(&request.capability)?;
        let object = parse_customer_auth_entity(&request.object)?;
        let subject = customer_hook_auth_subject(self.execution.principal.principal_id.as_deref());
        let authorizer = Arc::clone(&self.state.route_authorizer);
        let allowed = run_customer_hook_future(async move {
            authorizer.check_capability(&subject, capability, &object).await
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthCheckResult {
            allowed,
            explanation: (!allowed).then(|| {
                format!(
                    "live auth denied `{}` for `{}`",
                    capability.as_str(),
                    request.object
                )
            }),
        })
    }

    fn explain_denial(
        &self,
        request: &AuthExplainRequest,
    ) -> Result<AuthExplanation, BackendError> {
        let capability = parse_customer_capability(&request.capability)?;
        let object = parse_customer_auth_entity(&request.object)?;
        let Some(explainer) = self.state.auth_explainer.clone() else {
            return Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "auth.explain.unavailable",
                "Runtime auth explanations are disabled for this installation.",
            ));
        };
        let explain_request = davenda_auth::LiveAuthExplainRequest {
            subject: customer_hook_auth_subject(self.execution.principal.principal_id.as_deref()),
            capability,
            object,
            options: davenda_auth::ExplainOptions::default(),
        };
        let explanation = run_customer_hook_future(async move {
            explainer.explain_capability(&explain_request).await
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthExplanation {
            summary: format!(
                "{} `{}` on `{}`",
                if explanation.decision.is_allowed() {
                    "allow"
                } else {
                    "deny"
                },
                explanation.capability.as_str(),
                explanation.object
            ),
            traces: vec![format!("{:?}", explanation.trace)],
        })
    }
}

fn cms_page_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("cms.pages.index", summary.into());
    for field in [
        "page_id",
        "page_title",
        "page_slug",
        "page_summary",
        "page_body_html",
    ] {
        let value = storefront_form_field_value(execution, field);
        if !value.is_empty() {
            state = state.with_field_value(field, value);
        }
    }
    state
}

fn order_refund_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("commerce.order-detail", summary.into());
    for field in ["order_id", "reason"] {
        state = state.with_field_value(field, storefront_form_field_value(execution, field));
    }
    state
}

fn record_operator_audit(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    action: &str,
    resource_kind: &str,
    resource_id: &str,
    outcome: &str,
    detail: &str,
) -> Result<(), RuntimeServerError> {
    let capability = match execution.route.auth {
        RouteAuthGate::Capability(capability) => capability.to_string(),
        RouteAuthGate::Session => "session".to_string(),
        RouteAuthGate::Public => "public".to_string(),
    };
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("action", action)
        .append_pair("route", execution.route.route_name.as_str())
        .append_pair("capability", capability.as_str())
        .append_pair("outcome", outcome);
    if !resource_kind.trim().is_empty() {
        serializer.append_pair("resource_kind", resource_kind);
    }
    if !resource_id.trim().is_empty() {
        serializer.append_pair("resource_id", resource_id);
    }
    if !detail.trim().is_empty() {
        serializer.append_pair("detail", detail);
    }
    let kind = serializer.finish();
    match state
        .wasm_host
        .record_operator_audit(
            kind.clone(),
            execution.customer_app.as_str(),
            Some(execution.trace.request_id.as_str()),
            execution.principal.principal_id.as_deref(),
        )
    {
        Ok(()) => Ok(()),
        Err(_metadata_reason) => record_admin_audit_entry(
            &state.plan,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            execution
                .principal
                .principal_id
                .clone()
                .unwrap_or_else(|| "anonymous".to_string()),
            kind,
        )
        .map_err(|fallback_reason| RuntimeServerError::Configuration {
            reason: format!(
                "failed to persist operator audit entry for `{action}` using the local fallback: {fallback_reason}"
            ),
        }),
    }
}

fn cms_navigation_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("cms.navigation.index", summary.into());
    for (name, values) in &execution.form_fields {
        if (name.starts_with("nav_label_")
            || name.starts_with("nav_href_")
            || name == "new_nav_label"
            || name == "new_nav_href")
            && !values.is_empty()
        {
            state = state.with_field_value(name.clone(), values[0].clone());
        }
    }
    state
}

fn cms_redirect_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("cms.redirects.index", summary.into());
    for (name, values) in &execution.form_fields {
        if (name.starts_with("redirect_from_")
            || name.starts_with("redirect_to_")
            || name.starts_with("redirect_permanent_")
            || name == "new_redirect_from"
            || name == "new_redirect_to"
            || name == "new_redirect_permanent")
            && !values.is_empty()
        {
            state = state.with_field_value(name.clone(), values[0].clone());
        }
    }
    if execution.form_fields.contains_key("new_redirect_permanent") {
        state = state.with_field_value("new_redirect_permanent", "yes");
    }
    state
}

fn storefront_cart_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("commerce.cart", summary.into());
    for (name, values) in &execution.form_fields {
        if name.starts_with("quantity_") {
            if let Some(value) = values.first() {
                state = state.with_field_value(name.clone(), value.clone());
            }
        }
    }
    state
}

fn runtime_customer_request_context(
    state: &RuntimeServerState,
    execution: &RequestExecution,
) -> SdkRequestContext {
    let environment = match state.plan.config.app.environment {
        davenda_config::Environment::Development => "development",
        davenda_config::Environment::Staging => "staging",
        davenda_config::Environment::Production => "production",
    };
    let customer_app = match execution.locale.trim() {
        "" => SdkCustomerAppContext::new(execution.customer_app.clone(), environment.to_owned()),
        locale => {
            SdkCustomerAppContext::new(execution.customer_app.clone(), environment.to_owned())
                .with_locale(locale.to_string())
        }
    };
    let principal = execution
        .principal
        .principal_id
        .as_deref()
        .map(|principal_id| SdkPrincipalContext {
            kind: SdkPrincipalKind::User,
            id: Some(principal_id.to_string()),
        })
        .unwrap_or_else(SdkPrincipalContext::anonymous);
    let trace = SdkTraceContext::new(execution.trace.request_id.clone())
        .with_request_id(execution.trace.request_id.clone());
    SdkRequestContext::new(customer_app, principal, trace)
}

#[derive(Debug)]
struct RuntimeRequestAuditFacade<'a> {
    plan: &'a RuntimePlan,
    principal_id: Option<&'a str>,
    recorded_at_unix_seconds: u64,
}

impl AuditFacade for RuntimeRequestAuditFacade<'_> {
    fn record(&self, entry: AuditEntry) -> Result<(), BackendError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        serializer
            .append_pair("action", entry.action.as_str())
            .append_pair("resource_kind", entry.resource_kind.as_str())
            .append_pair("resource_id", entry.resource_id.as_str())
            .append_pair("outcome", entry.outcome.as_str());
        if let Some(detail) = entry.detail.as_deref() {
            serializer.append_pair("detail", detail);
        }
        for (key, value) in &entry.metadata {
            serializer.append_pair(&format!("meta.{key}"), value);
        }

        record_admin_audit_entry(
            self.plan,
            self.recorded_at_unix_seconds.min(i64::MAX as u64) as i64,
            self.principal_id.unwrap_or("anonymous"),
            serializer.finish(),
        )
        .map_err(|reason| {
            BackendError::new(
                BackendErrorKind::Internal,
                "audit.record.failed",
                "Failed to persist the customer hook audit entry.",
            )
            .with_detail(reason)
        })
    }
}

#[derive(Debug)]
struct RuntimeCmsRepositoryFacade<'a> {
    workspace: &'a CmsAdminWorkspace,
}

impl RepositoryFacade for RuntimeCmsRepositoryFacade<'_> {
    fn read(&self, query: &RepositoryQuery) -> Result<RepositoryRecordSet, BackendError> {
        if query.repository != "cms.pages" {
            return Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "repository.read.unsupported",
                format!(
                    "Runtime customer CMS hooks only expose `cms.pages` reads; `{}` is not available.",
                    query.repository
                ),
            ));
        }

        let records = self
            .workspace
            .pages
            .iter()
            .filter(|page| {
                query.key.as_deref().map_or(true, |key| {
                    page.id == key
                        || page.draft.slug == key
                        || page.live.as_ref().is_some_and(|live| live.slug == key)
                })
            })
            .map(|page| {
                let mut fields = BTreeMap::new();
                fields.insert("title".to_string(), page.draft.title.clone());
                fields.insert("slug".to_string(), page.draft.slug.clone());
                fields.insert("summary".to_string(), page.draft.summary.clone());
                fields.insert("body_html".to_string(), page.draft.body_html.clone());
                fields.insert("status".to_string(), page.status_label().to_string());
                if let Some(live_path) = page.live_path() {
                    fields.insert("live_path".to_string(), live_path);
                }
                RepositoryRecord {
                    id: page.id.clone(),
                    fields,
                }
            })
            .collect();

        Ok(RepositoryRecordSet {
            repository: query.repository.clone(),
            records,
        })
    }

    fn write(&self, change: RepositoryWrite) -> Result<RepositoryWriteReceipt, BackendError> {
        Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "repository.write.unsupported",
            format!(
                "Runtime customer CMS hooks are validation-only; repository writes to `{}` are not supported during publish.",
                change.repository
            ),
        ))
    }
}

#[derive(Debug)]
struct RuntimeCustomerJobsFacade<'a> {
    plan: &'a RuntimePlan,
    trace_id: &'a str,
    now: BrowserInstant,
}

impl JobsFacade for RuntimeCustomerJobsFacade<'_> {
    fn enqueue(
        &self,
        request: davenda_customer_sdk::JobRequest,
    ) -> Result<JobReceipt, BackendError> {
        let mut host = self
            .plan
            .jobs_host(format!("customer-hooks-{}", self.trace_id))
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Unavailable,
                    "jobs.host.unavailable",
                    "Runtime jobs coordination is unavailable for the customer webhook hook.",
                )
                .with_detail(error.to_string())
            })?;
        let Some(definition) = host
            .registered_jobs
            .iter()
            .find(|definition| definition.contract.name == request.job_name)
            .cloned()
        else {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "jobs.name.unknown",
                format!(
                    "Customer webhook requested unknown runtime job `{}`.",
                    request.job_name
                ),
            ));
        };
        if definition.queue.as_str() != request.queue {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "jobs.queue.mismatch",
                format!(
                    "Customer webhook requested queue `{}` for `{}`, but the registered runtime job uses `{}`.",
                    request.queue,
                    request.job_name,
                    definition.queue
                ),
            ));
        }

        let mut dispatch =
            JobDispatchRequest::new(request.job_name.clone(), request.payload_description).map_err(
                |error| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "jobs.dispatch.invalid",
                        "Customer webhook requested an invalid runtime job dispatch.",
                    )
                    .with_detail(error.to_string())
                },
            )?;
        if let Some(idempotency_key) = request.idempotency_key {
            dispatch = dispatch.with_idempotency_key(idempotency_key).map_err(|error| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "jobs.idempotency.invalid",
                    "Customer webhook requested an invalid idempotency key.",
                )
                .with_detail(error.to_string())
            })?;
        }
        let enqueued = host
            .enqueue_job(
                dispatch,
                JobInstant::from_unix_seconds(self.now.as_unix_seconds()),
            )
            .map_err(|error| {
                BackendError::new(
                    BackendErrorKind::Unavailable,
                    "jobs.enqueue.failed",
                    "Runtime could not enqueue the customer webhook follow-up job.",
                )
                .with_detail(error.to_string())
            })?;

        Ok(JobReceipt {
            queue: definition.queue.to_string(),
            job_id: enqueued.to_string(),
        })
    }
}

#[derive(Debug)]
struct RuntimeCustomerOutboundHttpFacade<'a> {
    wasm_host: &'a WasmHost,
}

impl OutboundHttpFacade for RuntimeCustomerOutboundHttpFacade<'_> {
    fn send(&self, request: OutboundHttpRequest) -> Result<OutboundHttpResponse, BackendError> {
        self.wasm_host.send_outbound_http(&request).map_err(|reason| {
            BackendError::new(
                BackendErrorKind::Unavailable,
                "http.send.failed",
                format!(
                    "Runtime could not execute approved outbound HTTP integration `{}`.",
                    request.integration
                ),
            )
            .with_detail(reason)
        })
    }
}

fn checkout_order_draft(
    state: &RuntimeServerState,
    session_id: &str,
    principal_id: Option<&str>,
    payment: &StorefrontPaymentInput,
) -> Result<OrderDraft, RuntimeServerError> {
    let snapshot = state.storefront.snapshot(session_id, principal_id)?;
    let lines = snapshot
        .cart
        .lines
        .iter()
        .map(|line| {
            let collection_handle = state
                .plan
                .storefront_catalog
                .product_by_sku_or_handle(&line.sku)
                .map(|product| product.collection_handle.clone());
            OrderLineDraft {
                sku: line.sku.clone(),
                title: line.title.clone(),
                quantity: line.quantity,
                unit_price: MoneyAmount::new(line.currency.clone(), line.unit_price_minor),
                product_kind: line.product_kind.clone(),
                collection_handle,
                entitlement_key: line.entitlement_key.clone(),
                metadata: BTreeMap::new(),
            }
        })
        .collect::<Vec<_>>();
    let mut metadata = BTreeMap::new();
    metadata.insert("session_id".to_string(), session_id.to_string());
    metadata.insert(
        "payment_method".to_string(),
        payment.method.trim().to_ascii_lowercase(),
    );
    metadata.insert(
        "checkout_email".to_string(),
        payment.checkout_email.trim().to_string(),
    );
    Ok(OrderDraft {
        order_id: format!("draft:{}", payment.intent_reference),
        currency_code: snapshot.cart.currency.clone(),
        subtotal: MoneyAmount::new(snapshot.cart.currency.clone(), snapshot.cart.subtotal_minor),
        total: MoneyAmount::new(snapshot.cart.currency.clone(), snapshot.cart.subtotal_minor),
        lines,
        metadata,
    })
}

fn customer_checkout_error_summary(error: &BackendError) -> Cow<'static, str> {
    match error.kind() {
        BackendErrorKind::InvalidInput
        | BackendErrorKind::Forbidden
        | BackendErrorKind::Conflict
        | BackendErrorKind::Unauthorized => Cow::Owned(error.message().to_string()),
        BackendErrorKind::Unsupported => Cow::Borrowed(
            "Checkout could not continue because a required customer backend feature is not available in this runtime yet.",
        ),
        BackendErrorKind::Unavailable | BackendErrorKind::Timeout | BackendErrorKind::Internal => {
            Cow::Borrowed("Checkout is temporarily unavailable. Review the basket and try again.")
        }
    }
}

fn customer_cms_publish_error_summary(error: &BackendError) -> Cow<'static, str> {
    match error.kind() {
        BackendErrorKind::InvalidInput
        | BackendErrorKind::Forbidden
        | BackendErrorKind::Conflict
        | BackendErrorKind::Unauthorized => Cow::Owned(error.message().to_string()),
        BackendErrorKind::Unsupported => Cow::Borrowed(
            "Publishing could not continue because a required customer backend feature is not available in this runtime yet.",
        ),
        BackendErrorKind::Unavailable | BackendErrorKind::Timeout | BackendErrorKind::Internal => {
            Cow::Borrowed("Publishing is temporarily unavailable. Review the page and try again.")
        }
    }
}

fn customer_hook_request_headers(execution: &RequestExecution) -> Headers {
    BTreeMap::from([
        (
            "x-davenda-customer-app".to_string(),
            execution.customer_app.clone(),
        ),
        (
            "x-davenda-request-id".to_string(),
            execution.trace.request_id.clone(),
        ),
        (
            "x-davenda-route".to_string(),
            execution.route.route_name.clone(),
        ),
    ])
}

fn cms_page_draft_from_workspace(page: &CmsAdminPage, locale: &str) -> CmsPageDraft {
    let mut metadata = BTreeMap::new();
    metadata.insert("status".to_string(), page.status_label().to_string());
    metadata.insert(
        "published_once".to_string(),
        if page.published_once { "true" } else { "false" }.to_string(),
    );
    if let Some(live_path) = page.live_path() {
        metadata.insert("live_path".to_string(), live_path);
    }
    CmsPageDraft {
        page_id: page.id.clone(),
        slug: page.draft.slug.clone(),
        title: page.draft.title.clone(),
        summary: page.draft.summary.clone(),
        body_html: page.draft.body_html.clone(),
        locale: (!locale.trim().is_empty()).then(|| locale.to_string()),
        metadata,
    }
}

fn validate_cms_publish_with_customer_hooks(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    workspace: &CmsAdminWorkspace,
    page_id: &str,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if state.plan.customer_hooks.cms.is_empty() {
        return Ok(None);
    }

    let page = workspace.selected_page(Some(page_id)).ok_or_else(|| {
        RuntimeServerError::Configuration {
            reason: format!("CMS page `{page_id}` was not found during publish validation"),
        }
    })?;
    let context = runtime_customer_request_context(state, execution);
    let draft = cms_page_draft_from_workspace(page, execution.locale.as_str());
    let repositories = RuntimeCmsRepositoryFacade { workspace };
    let audit = RuntimeRequestAuditFacade {
        plan: &state.plan,
        principal_id: execution.principal.principal_id.as_deref(),
        recorded_at_unix_seconds: now.as_unix_seconds(),
    };

    for hook in &state.plan.customer_hooks.cms {
        match hook.validate_page_publish(&context, &draft, &repositories, &audit) {
            Ok(CmsPublishDecision::Allow) => {}
            Ok(CmsPublishDecision::Reject { message, .. }) => {
                let form_state = cms_page_form_state_from_execution(execution, message);
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some(format!("/admin/pages?page={page_id}")));
            }
            Err(error) => {
                let form_state = cms_page_form_state_from_execution(
                    execution,
                    customer_cms_publish_error_summary(&error),
                );
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some(format!("/admin/pages?page={page_id}")));
            }
        }
    }

    Ok(None)
}

fn execute_verified_webhook_customer_hooks(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    webhook: &VerifiedWebhook,
    now: BrowserInstant,
) -> Result<(), RuntimeServerError> {
    if state.plan.customer_hooks.verified_webhooks.is_empty() {
        return Ok(());
    }

    let context = runtime_customer_request_context(state, execution);
    let http = RuntimeCustomerOutboundHttpFacade {
        wasm_host: &state.wasm_host,
    };
    let jobs = RuntimeCustomerJobsFacade {
        plan: &state.plan,
        trace_id: execution.trace.request_id.as_str(),
        now,
    };
    let audit = RuntimeRequestAuditFacade {
        plan: &state.plan,
        principal_id: execution.principal.principal_id.as_deref(),
        recorded_at_unix_seconds: now.as_unix_seconds(),
    };

    for hook in &state.plan.customer_hooks.verified_webhooks {
        match hook.handle_verified_webhook(&context, webhook, &http, &jobs, &audit) {
            Ok(WebhookHandlingResult::Accepted { detail }) => {
                audit.record(
                    AuditEntry::new(
                        "customer-plugin.verified-webhook",
                        "webhook",
                        format!("{}:{}", webhook.source, webhook.event),
                        "accepted",
                    )
                    .with_detail(detail.unwrap_or_else(|| {
                        "customer hook accepted verified webhook".to_string()
                    })),
                )
                .map_err(|error| RuntimeServerError::CustomerHookFailed {
                    surface: "verified-webhook",
                    reason: error.to_string(),
                })?;
            }
            Ok(WebhookHandlingResult::Rejected { code, message }) => {
                audit.record(
                    AuditEntry::new(
                        "customer-plugin.verified-webhook",
                        "webhook",
                        format!("{}:{}", webhook.source, webhook.event),
                        "rejected",
                    )
                    .with_detail(format!("{code}: {message}")),
                )
                .map_err(|error| RuntimeServerError::CustomerHookFailed {
                    surface: "verified-webhook",
                    reason: error.to_string(),
                })?;
                return Err(RuntimeServerError::CustomerHookRejected {
                    surface: "verified-webhook",
                    code,
                    message,
                });
            }
            Err(error) => {
                audit.record(
                    AuditEntry::new(
                        "customer-plugin.verified-webhook",
                        "webhook",
                        format!("{}:{}", webhook.source, webhook.event),
                        "failed",
                    )
                    .with_detail(error.to_string()),
                )
                .map_err(|audit_error| RuntimeServerError::CustomerHookFailed {
                    surface: "verified-webhook",
                    reason: audit_error.to_string(),
                })?;
                return Err(RuntimeServerError::CustomerHookFailed {
                    surface: "verified-webhook",
                    reason: error.to_string(),
                });
            }
        }
    }

    Ok(())
}

fn review_checkout_with_customer_hooks(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    session_id: &str,
    payment: &StorefrontPaymentInput,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if state.plan.customer_hooks.checkout.is_empty() {
        return Ok(None);
    }

    let context = runtime_customer_request_context(state, execution);
    let order = checkout_order_draft(
        state,
        session_id,
        execution.principal.principal_id.as_deref(),
        payment,
    )?;
    let commerce = RuntimeCheckoutCommerceFacade {
        catalog: &state.plan.storefront_catalog,
    };
    let auth = RuntimeCheckoutAuthFacade { state, execution };
    let audit = RuntimeRequestAuditFacade {
        plan: &state.plan,
        principal_id: execution.principal.principal_id.as_deref(),
        recorded_at_unix_seconds: now.as_unix_seconds(),
    };
    let mut adjustment_messages = Vec::new();

    for hook in &state.plan.customer_hooks.checkout {
        match hook.review_order(&context, &order, &commerce, &auth, &audit) {
            Ok(OrderReviewDecision::Approved) => {}
            Ok(OrderReviewDecision::Adjusted(adjustment)) => {
                adjustment_messages.push(adjustment.reason);
            }
            Ok(OrderReviewDecision::Rejected(rejection)) => {
                let form_state =
                    storefront_checkout_form_state_from_execution(execution, rejection.message);
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some("/checkout".to_string()));
            }
            Err(error) => {
                let form_state = storefront_checkout_form_state_from_execution(
                    execution,
                    customer_checkout_error_summary(&error),
                );
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some("/checkout".to_string()));
            }
        }
    }

    for message in adjustment_messages {
        push_storefront_flash(state, response_cookies, FlashLevel::Info, message)?;
    }

    Ok(None)
}

fn catalog_admin_product_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("commerce.catalog-admin", summary.into());
    for field in [
        "catalog_entity",
        "product_handle",
        "product_title",
        "product_summary",
        "product_price",
        "product_collection_handle",
    ] {
        let value = storefront_form_field_value(execution, field);
        if !value.is_empty() {
            state = state.with_field_value(field, value);
        }
    }
    if storefront_form_field_value(execution, "product_visible") == "yes" {
        state = state.with_field_value("product_visible", "yes");
    }
    state
}

fn catalog_admin_collection_form_state_from_execution(
    execution: &RequestExecution,
    summary: impl Into<String>,
) -> StorefrontFormState {
    let mut state = StorefrontFormState::new("commerce.catalog-admin", summary.into());
    for field in [
        "catalog_entity",
        "collection_handle",
        "collection_title",
        "collection_label",
        "collection_summary",
    ] {
        let value = storefront_form_field_value(execution, field);
        if !value.is_empty() {
            state = state.with_field_value(field, value);
        }
    }
    if storefront_form_field_value(execution, "collection_visible") == "yes" {
        state = state.with_field_value("collection_visible", "yes");
    }
    state
}

fn parse_decimal_price_minor(value: &str) -> Option<i64> {
    let trimmed = value.trim().trim_start_matches('£');
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }
    let mut parts = trimmed.split('.');
    let pounds = parts.next()?;
    let pence = parts.next().unwrap_or("00");
    if parts.next().is_some()
        || pounds.is_empty()
        || !pounds.chars().all(|ch| ch.is_ascii_digit())
        || !pence.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let pence = match pence.len() {
        0 => "00".to_string(),
        1 => format!("{pence}0"),
        2 => pence.to_string(),
        _ => return None,
    };
    let pounds = pounds.parse::<i64>().ok()?;
    let pence = pence.parse::<i64>().ok()?;
    let minor = pounds.checked_mul(100)?.checked_add(pence)?;
    (minor > 0).then_some(minor)
}

fn validated_catalog_admin_update_from_execution(
    execution: &RequestExecution,
) -> Result<CatalogAdminMutationInput, StorefrontFormState> {
    match execution_form_field(execution, "catalog_entity").unwrap_or_default() {
        "product" => {
            let mut form_state = catalog_admin_product_form_state_from_execution(
                execution,
                "Fix the highlighted product fields and save again.",
            );
            let handle = storefront_form_field_value(execution, "product_handle");
            let title = storefront_form_field_value(execution, "product_title");
            let summary = storefront_form_field_value(execution, "product_summary");
            let price = storefront_form_field_value(execution, "product_price");
            let collection_handle =
                storefront_form_field_value(execution, "product_collection_handle");
            let is_visible = storefront_form_field_value(execution, "product_visible") == "yes";
            let mut has_errors = false;
            if handle.trim().is_empty() {
                has_errors = true;
                form_state = form_state.with_field_error(
                    "product_handle",
                    "Refresh the page and try again before saving this product.",
                );
            }
            if title.trim().is_empty() {
                has_errors = true;
                form_state = form_state.with_field_error("product_title", "Enter a product title.");
            }
            if summary.trim().is_empty() {
                has_errors = true;
                form_state =
                    form_state.with_field_error("product_summary", "Enter a product summary.");
            }
            if collection_handle.trim().is_empty() {
                has_errors = true;
                form_state = form_state.with_field_error(
                    "product_collection_handle",
                    "Choose a collection for this product.",
                );
            }
            let price_minor = match parse_decimal_price_minor(&price) {
                Some(price_minor) => price_minor,
                None => {
                    has_errors = true;
                    form_state = form_state.with_field_error(
                        "product_price",
                        "Enter a positive GBP price such as 29.00.",
                    );
                    0
                }
            };
            if has_errors {
                return Err(form_state);
            }
            Ok(CatalogAdminMutationInput::Product(
                crate::storefront::StorefrontCatalogProductUpdate {
                    handle,
                    title,
                    summary,
                    price_minor,
                    collection_handle,
                    is_visible,
                },
            ))
        }
        "collection" => {
            let mut form_state = catalog_admin_collection_form_state_from_execution(
                execution,
                "Fix the highlighted collection fields and save again.",
            );
            let handle = storefront_form_field_value(execution, "collection_handle");
            let title = storefront_form_field_value(execution, "collection_title");
            let label = storefront_form_field_value(execution, "collection_label");
            let summary = storefront_form_field_value(execution, "collection_summary");
            let is_visible = storefront_form_field_value(execution, "collection_visible") == "yes";
            let mut has_errors = false;
            if handle.trim().is_empty() {
                has_errors = true;
                form_state = form_state.with_field_error(
                    "collection_handle",
                    "Refresh the page and try again before saving this collection.",
                );
            }
            if title.trim().is_empty() {
                has_errors = true;
                form_state =
                    form_state.with_field_error("collection_title", "Enter a collection title.");
            }
            if label.trim().is_empty() {
                has_errors = true;
                form_state =
                    form_state.with_field_error("collection_label", "Enter a merchandising label.");
            }
            if summary.trim().is_empty() {
                has_errors = true;
                form_state = form_state
                    .with_field_error("collection_summary", "Enter a collection summary.");
            }
            if has_errors {
                return Err(form_state);
            }
            Ok(CatalogAdminMutationInput::Collection(
                crate::storefront::StorefrontCatalogCollectionUpdate {
                    handle,
                    title,
                    label,
                    summary,
                    is_visible,
                },
            ))
        }
        _ => Err(StorefrontFormState::new(
            "commerce.catalog-admin",
            "Refresh the catalog admin page and try the save action again.",
        )),
    }
}

fn validated_cart_quantities_from_execution(
    execution: &RequestExecution,
) -> Result<BTreeMap<String, u32>, StorefrontFormState> {
    let mut quantities = BTreeMap::new();
    let mut form_state = storefront_cart_form_state_from_execution(
        execution,
        "Fix the highlighted cart quantities and try again.",
    );
    let mut has_errors = false;
    for (name, values) in &execution.form_fields {
        let Some(product_slug) = name.strip_prefix("quantity_") else {
            continue;
        };
        let raw = values.first().cloned().unwrap_or_default();
        match raw.trim().parse::<u32>() {
            Ok(quantity) => {
                quantities.insert(product_slug.to_string(), quantity);
            }
            Err(_) => {
                has_errors = true;
                form_state = form_state
                    .with_field_error(name.clone(), "Enter a whole-number quantity for this line.");
            }
        }
    }
    if has_errors {
        Err(form_state)
    } else {
        Ok(quantities)
    }
}

fn validated_storefront_payment_input_from_execution(
    state: &RuntimeServerState,
    execution: &RequestExecution,
) -> Result<StorefrontPaymentInput, StorefrontFormState> {
    let hosted_checkout = configured_commerce_payment_provider(&state.plan.config)
        .map(|provider| provider.uses_hosted_checkout())
        .unwrap_or(false);
    let checkout_email = execution_form_field(execution, "checkout_email")
        .or_else(|| execution_form_field(execution, "checkoutEmail"))
        .or_else(|| execution_form_field(execution, "email"))
        .or_else(|| execution_form_field(execution, "billing_email"))
        .unwrap_or_default()
        .trim()
        .to_string();
    let last4 = execution_form_field(execution, "payment_last4")
        .or_else(|| execution_form_field(execution, "paymentLast4"))
        .or_else(|| execution_form_field(execution, "card_last4"))
        .unwrap_or_default()
        .trim()
        .to_string();
    let method = execution_form_field(execution, "payment_method")
        .or_else(|| execution_form_field(execution, "paymentMethod"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| (!last4.is_empty()).then(|| "card".to_string()))
        .unwrap_or_default();
    let mut form_state = storefront_checkout_form_state_from_execution(
        execution,
        "There is a problem with your checkout details.",
    );
    let mut has_errors = false;
    if checkout_email.is_empty() {
        has_errors = true;
        form_state = form_state.with_field_error(
            "checkout_email",
            "Enter the email address for order confirmation.",
        );
    }
    if method.is_empty() && !hosted_checkout {
        has_errors = true;
        form_state = form_state.with_field_error(
            "payment_method",
            "Choose or confirm a payment method before placing the order.",
        );
    }
    if method == "card"
        && !hosted_checkout
        && (last4.len() != 4 || !last4.chars().all(|character| character.is_ascii_digit()))
    {
        has_errors = true;
        form_state = form_state.with_field_error(
            "payment_last4",
            "Enter the final 4 digits for the payment card.",
        );
    }
    if execution_form_field(execution, "checkout_intent")
        .or_else(|| execution_form_field(execution, "payment_intent"))
        .or_else(|| execution_form_field(execution, "payment_reference"))
        .or_else(|| execution_form_field(execution, "paymentReference"))
        .is_none()
    {
        has_errors = true;
        form_state = form_state
            .with_summary("Refresh checkout before placing the order.")
            .with_field_error(
                "checkout_intent",
                "Refresh checkout and try again before placing the order.",
            );
    }
    if execution_form_field(execution, "terms_accepted").is_none() {
        has_errors = true;
        form_state = form_state.with_field_error(
            "terms_accepted",
            "Review the basket and confirm the final total before placing the order.",
        );
    }
    if has_errors {
        return Err(form_state);
    }
    let intent_reference = execution_form_field(execution, "checkout_intent")
        .or_else(|| execution_form_field(execution, "payment_intent"))
        .or_else(|| execution_form_field(execution, "payment_reference"))
        .or_else(|| execution_form_field(execution, "paymentReference"))
        .unwrap_or_default();
    let method = if method.is_empty() && hosted_checkout {
        "card".to_string()
    } else {
        method
    };
    let payment = if hosted_checkout && last4.is_empty() {
        StorefrontPaymentInput::hosted(method, checkout_email, intent_reference)
    } else {
        StorefrontPaymentInput::new(
            method,
            checkout_email,
            (!last4.is_empty()).then_some(last4),
            intent_reference,
        )
    };
    payment.map_err(|error| {
        let mut form_state = storefront_checkout_form_state_from_execution(
            execution,
            "There is a problem with your checkout details.",
        );
        let (field, message) = match error {
            StorefrontStateError::MissingPaymentMethod => (
                "payment_method",
                "Choose or confirm a payment method before placing the order.",
            ),
            StorefrontStateError::MissingCheckoutEmail => (
                "checkout_email",
                "Enter the email address for order confirmation.",
            ),
            StorefrontStateError::InvalidPaymentLast4 => (
                "payment_last4",
                "Enter the final 4 digits for the payment card.",
            ),
            StorefrontStateError::MissingPaymentIntent => (
                "checkout_intent",
                "Refresh checkout and try again before placing the order.",
            ),
            _ => (
                "checkout_email",
                "Update the checkout details and try again.",
            ),
        };
        form_state = form_state.with_field_error(field, message);
        form_state
    })
}

fn validated_payment_webhook_from_execution(
    state: &RuntimeServerState,
    execution: &RequestExecution,
) -> Result<VerifiedPaymentWebhook, RuntimeServerError> {
    let provider = execution_form_field(execution, "provider")
        .unwrap_or("generic")
        .trim()
        .to_ascii_lowercase();
    if let Some(configured_provider) = configured_commerce_payment_provider(&state.plan.config) {
        if provider != configured_provider.code {
            return Err(RuntimeServerError::Storefront(
                StorefrontStateError::UnexpectedPaymentWebhookProvider {
                    expected: configured_provider.code,
                    received: provider,
                },
            ));
        }
    }
    let event = execution_form_field(execution, "event")
        .or_else(|| execution_form_field(execution, "payment_event"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeServerError::Storefront(StorefrontStateError::UnknownPaymentWebhookEvent {
                event: "<missing>".to_string(),
            })
        })?;
    let payment_reference = execution_form_field(execution, "payment_reference")
        .or_else(|| execution_form_field(execution, "paymentReference"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeServerError::Storefront(StorefrontStateError::UnknownPaymentReference {
                payment_reference: "<missing>".to_string(),
            })
        })?;
    let signature = execution_form_field(execution, "signature")
        .or_else(|| execution_form_field(execution, "webhook_signature"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(RuntimeServerError::Storefront(
            StorefrontStateError::InvalidPaymentWebhookSignature,
        ))?;
    let secret = state
        .payment_webhook_secret
        .as_deref()
        .ok_or(RuntimeServerError::Storefront(
            StorefrontStateError::MissingPaymentWebhookSecret,
        ))?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
        RuntimeServerError::Storefront(StorefrontStateError::MissingPaymentWebhookSecret)
    })?;
    mac.update(provider.as_bytes());
    mac.update(b":");
    mac.update(event.as_bytes());
    mac.update(b":");
    mac.update(payment_reference.as_bytes());
    let provided_signature = decode_hex_signature(signature).ok_or(
        RuntimeServerError::Storefront(StorefrontStateError::InvalidPaymentWebhookSignature),
    )?;
    if mac.verify_slice(&provided_signature).is_err() {
        return Err(RuntimeServerError::Storefront(
            StorefrontStateError::InvalidPaymentWebhookSignature,
        ));
    }
    Ok(VerifiedPaymentWebhook {
        event,
        payment_reference,
    })
}

fn storefront_payment_input_from_execution(
    execution: &RequestExecution,
) -> Result<StorefrontPaymentInput, RuntimeServerError> {
    let intent_reference = execution_form_field(execution, "checkout_intent")
        .or_else(|| execution_form_field(execution, "payment_intent"))
        .or_else(|| execution_form_field(execution, "payment_reference"))
        .or_else(|| execution_form_field(execution, "paymentReference"));
    let last4 = execution_form_field(execution, "payment_last4")
        .or_else(|| execution_form_field(execution, "paymentLast4"))
        .or_else(|| execution_form_field(execution, "card_last4"))
        .map(str::to_string);
    let method = execution_form_field(execution, "payment_method")
        .or_else(|| execution_form_field(execution, "paymentMethod"))
        .map(str::to_string)
        .or_else(|| last4.as_ref().map(|_| "card".to_string()));
    let checkout_email = execution_form_field(execution, "checkout_email")
        .or_else(|| execution_form_field(execution, "checkoutEmail"))
        .or_else(|| execution_form_field(execution, "email"))
        .or_else(|| execution_form_field(execution, "billing_email"));
    StorefrontPaymentInput::new(
        method.unwrap_or_default(),
        checkout_email.unwrap_or_default(),
        last4,
        intent_reference.unwrap_or_default(),
    )
    .map_err(RuntimeServerError::Storefront)
}

fn apply_native_cms_admin_mutations(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if !CMS_ADMIN_NATIVE_MUTATION_ROUTES.contains(&execution.route.route_name.as_str()) {
        return Ok(None);
    }

    let mut workspace = CmsAdminWorkspace::load(&state.plan).map_err(|reason| {
        RuntimeServerError::Configuration {
            reason: format!("failed to load CMS admin workspace: {reason}"),
        }
    })?;

    match execution.route.route_name.as_str() {
        "cms.pages.save-draft" => {
            let page_input = CmsAdminPageInput {
                page_id: execution_form_field(execution, "page_id").map(str::to_string),
                title: storefront_form_field_value(execution, "page_title"),
                slug: storefront_form_field_value(execution, "page_slug"),
                summary: storefront_form_field_value(execution, "page_summary"),
                body_html: storefront_form_field_value(execution, "page_body_html"),
            };
            let page_id = match workspace.save_page_draft(page_input, now.as_unix_seconds()) {
                Ok(page_id) => page_id,
                Err(reason) => {
                    let mut form_state =
                        cms_page_form_state_from_execution(execution, reason.clone());
                    for field in ["page_title", "page_slug", "page_summary", "page_body_html"] {
                        form_state = form_state.with_field_error(field, reason.clone());
                    }
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/admin/pages".to_string()));
                }
            };
            workspace
                .save(&state.plan)
                .map_err(|reason| RuntimeServerError::Configuration {
                    reason: format!("failed to persist CMS page draft: {reason}"),
                })?;
            record_operator_audit(
                state,
                execution,
                "cms.pages.save-draft",
                "page",
                page_id.as_str(),
                "succeeded",
                &format!(
                    "Saved draft `{}` at slug `{}`.",
                    storefront_form_field_value(execution, "page_title"),
                    storefront_form_field_value(execution, "page_slug"),
                ),
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                "Draft saved. Preview and publish when ready.",
            )?;
            return Ok(Some(format!("/admin/pages?page={page_id}")));
        }
        "cms.pages.publish" => {
            let page_id = if execution_form_field(execution, "page_title").is_some() {
                let page_input = CmsAdminPageInput {
                    page_id: execution_form_field(execution, "page_id").map(str::to_string),
                    title: storefront_form_field_value(execution, "page_title"),
                    slug: storefront_form_field_value(execution, "page_slug"),
                    summary: storefront_form_field_value(execution, "page_summary"),
                    body_html: storefront_form_field_value(execution, "page_body_html"),
                };
                match workspace.save_page_draft(page_input, now.as_unix_seconds()) {
                    Ok(page_id) => page_id,
                    Err(reason) => {
                        let mut form_state =
                            cms_page_form_state_from_execution(execution, reason.clone());
                        for field in ["page_title", "page_slug", "page_summary", "page_body_html"] {
                            form_state = form_state.with_field_error(field, reason.clone());
                        }
                        push_storefront_form_state(state, response_cookies, &form_state)?;
                        return Ok(Some("/admin/pages".to_string()));
                    }
                }
            } else {
                execution_form_field(execution, "page_id")
                    .map(str::to_string)
                    .ok_or_else(|| RuntimeServerError::Configuration {
                        reason: "missing page_id for publish".to_string(),
                    })?
            };
            if let Some(location) = validate_cms_publish_with_customer_hooks(
                state,
                execution,
                &workspace,
                &page_id,
                now,
                response_cookies,
            )? {
                return Ok(Some(location));
            }
            workspace
                .publish_page(&page_id, now.as_unix_seconds())
                .map_err(|reason| RuntimeServerError::Configuration { reason })?;
            workspace
                .save(&state.plan)
                .map_err(|reason| RuntimeServerError::Configuration {
                    reason: format!("failed to persist CMS publication: {reason}"),
                })?;
            let live_path = workspace
                .selected_page(Some(&page_id))
                .and_then(|page| page.live_path())
                .unwrap_or_else(|| "/pages/{slug}".to_string());
            record_operator_audit(
                state,
                execution,
                "cms.pages.publish",
                "page",
                page_id.as_str(),
                "succeeded",
                &format!("Published live route `{live_path}`."),
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                "Page published to the live /pages/{slug} surface.",
            )?;
            return Ok(Some(format!("/admin/pages?page={page_id}")));
        }
        "cms.pages.unpublish" => {
            let page_id = execution_form_field(execution, "page_id").ok_or_else(|| {
                RuntimeServerError::Configuration {
                    reason: "missing page_id for unpublish".to_string(),
                }
            })?;
            workspace
                .unpublish_page(page_id, now.as_unix_seconds())
                .map_err(|reason| RuntimeServerError::Configuration { reason })?;
            workspace
                .save(&state.plan)
                .map_err(|reason| RuntimeServerError::Configuration {
                    reason: format!("failed to persist CMS unpublish: {reason}"),
                })?;
            record_operator_audit(
                state,
                execution,
                "cms.pages.unpublish",
                "page",
                page_id,
                "succeeded",
                "Removed the page from its live route while preserving the draft.",
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Info,
                "Page removed from the live route but kept as a draft.",
            )?;
            return Ok(Some(format!("/admin/pages?page={page_id}")));
        }
        "cms.navigation.save" => {
            let items = match navigation_items_from_fields(&execution.form_fields) {
                Ok(items) => items,
                Err(reason) => {
                    let form_state =
                        cms_navigation_form_state_from_execution(execution, reason.clone())
                            .with_field_error("new_nav_label", reason);
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/admin/navigation".to_string()));
                }
            };
            if let Err(reason) = workspace.save_navigation(items) {
                let form_state =
                    cms_navigation_form_state_from_execution(execution, reason.clone())
                        .with_field_error("new_nav_label", reason);
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some("/admin/navigation".to_string()));
            }
            workspace
                .save(&state.plan)
                .map_err(|reason| RuntimeServerError::Configuration {
                    reason: format!("failed to persist CMS navigation: {reason}"),
                })?;
            record_operator_audit(
                state,
                execution,
                "cms.navigation.save",
                "navigation",
                "primary-navigation",
                "succeeded",
                &format!(
                    "Saved {} primary navigation items.",
                    workspace.navigation.len()
                ),
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                "Primary navigation updated for the live storefront shell.",
            )?;
            return Ok(Some("/admin/navigation".to_string()));
        }
        "cms.redirects.save" => {
            let redirects = match redirects_from_fields(&execution.form_fields) {
                Ok(redirects) => redirects,
                Err(reason) => {
                    let form_state =
                        cms_redirect_form_state_from_execution(execution, reason.clone())
                            .with_field_error("new_redirect_from", reason);
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/admin/redirects".to_string()));
                }
            };
            if let Err(reason) = workspace.save_redirects(redirects) {
                let form_state = cms_redirect_form_state_from_execution(execution, reason.clone())
                    .with_field_error("new_redirect_from", reason);
                push_storefront_form_state(state, response_cookies, &form_state)?;
                return Ok(Some("/admin/redirects".to_string()));
            }
            workspace
                .save(&state.plan)
                .map_err(|reason| RuntimeServerError::Configuration {
                    reason: format!("failed to persist CMS redirects: {reason}"),
                })?;
            record_operator_audit(
                state,
                execution,
                "cms.redirects.save",
                "redirects",
                "live-redirect-rules",
                "succeeded",
                &format!("Saved {} redirect rules.", workspace.redirects.len()),
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                "Redirect rules saved for unmatched live requests.",
            )?;
            return Ok(Some("/admin/redirects".to_string()));
        }
        _ => {}
    }

    Ok(None)
}

async fn apply_native_storefront_mutations(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if execution.route.route_name.as_str() == "commerce.payment-provider-webhook" {
        let verified = validated_payment_webhook_from_execution(state, execution)?;
        let provider = execution_form_field(execution, "provider")
            .unwrap_or("generic")
            .trim()
            .to_ascii_lowercase();
        let payload = serde_json::to_vec(&execution.form_fields).map_err(|error| {
            RuntimeServerError::Configuration {
                reason: format!(
                    "failed to encode verified webhook payload for customer hooks: {error}"
                ),
            }
        })?;
        let webhook = VerifiedWebhook {
            source: provider,
            event: verified.event.clone(),
            headers: customer_hook_request_headers(execution),
            content_type: Some("application/x-www-form-urlencoded".to_string()),
            payload,
        };
        execute_verified_webhook_customer_hooks(state, execution, &webhook, now)?;
        let receipt = state.storefront.apply_payment_webhook(
            verified.payment_reference.as_str(),
            verified.event.as_str(),
            now.as_unix_seconds(),
        )?;
        if receipt.needs_paid_event_dispatch {
            dispatch_paid_order_event(state, &receipt.order, now)?;
            state
                .storefront
                .mark_order_paid_event_dispatched(&receipt.order.order_id, now.as_unix_seconds())?;
        }
        return Ok(None);
    }

    let Some(session_id) = execution.session.session_id.as_deref() else {
        return Ok(None);
    };
    match execution.route.route_name.as_str() {
        "commerce.add-to-cart" => {
            let quantity = storefront_quantity_from_execution(execution);
            let sku = storefront_sku_from_execution(execution)?;
            let snapshot = state.storefront.add_to_cart(
                session_id,
                execution.principal.principal_id.as_deref(),
                sku.as_ref(),
                quantity,
                now.as_unix_seconds(),
            )?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                format!("Added {} to the cart ({})", sku, snapshot.cart.item_count),
            )?;
        }
        "commerce.cart-update" => {
            let quantities = match validated_cart_quantities_from_execution(execution) {
                Ok(quantities) => quantities,
                Err(form_state) => {
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/cart".to_string()));
                }
            };
            let mut snapshot = state
                .storefront
                .snapshot(session_id, execution.principal.principal_id.as_deref())?;
            for (sku, quantity) in quantities {
                snapshot = state.storefront.update_cart(
                    session_id,
                    execution.principal.principal_id.as_deref(),
                    &sku,
                    quantity,
                    now.as_unix_seconds(),
                )?;
            }
            let message = if snapshot.cart.lines.is_empty() {
                "Your cart is now empty.".to_string()
            } else {
                format!("Updated cart with {} line(s).", snapshot.cart.item_count)
            };
            push_storefront_flash(state, response_cookies, FlashLevel::Info, message)?;
        }
        "commerce.checkout-start" => {
            if let Ok(sku) = storefront_sku_from_execution(execution) {
                let quantity = storefront_quantity_from_execution(execution);
                let _ = state.storefront.add_to_cart(
                    session_id,
                    execution.principal.principal_id.as_deref(),
                    sku.as_ref(),
                    quantity,
                    now.as_unix_seconds(),
                )?;
            }
            match state.storefront.checkout_start(
                session_id,
                execution.principal.principal_id.as_deref(),
                now.as_unix_seconds(),
            ) {
                Ok(_) => {}
                Err(StorefrontStateError::EmptyCart { .. }) => {
                    let form_state = StorefrontFormState::new(
                        "commerce.cart",
                        "Add at least one item to the cart before starting checkout.",
                    );
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/cart".to_string()));
                }
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            }
        }
        "commerce.checkout-complete" => {
            let payment = match validated_storefront_payment_input_from_execution(state, execution)
            {
                Ok(payment) => payment,
                Err(form_state) => {
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/checkout".to_string()));
                }
            };
            if let Some(location) = review_checkout_with_customer_hooks(
                state,
                execution,
                session_id,
                &payment,
                now,
                response_cookies,
            )? {
                return Ok(Some(location));
            }
            let snapshot = match state.storefront.checkout_complete(
                session_id,
                execution.principal.principal_id.as_deref(),
                &payment,
                now.as_unix_seconds(),
            ) {
                Ok(snapshot) => snapshot,
                Err(
                    error @ (StorefrontStateError::CheckoutNotReady { .. }
                    | StorefrontStateError::EmptyCart { .. }
                    | StorefrontStateError::MissingPaymentIntent
                    | StorefrontStateError::PaymentIntentMismatch { .. }),
                ) => {
                    let summary = match &error {
                        StorefrontStateError::CheckoutNotReady { .. } => {
                            "Refresh checkout and review the basket before placing the order."
                        }
                        StorefrontStateError::EmptyCart { .. } => {
                            "Add at least one item to the cart before placing the order."
                        }
                        StorefrontStateError::MissingPaymentIntent => {
                            "Refresh checkout before placing the order."
                        }
                        StorefrontStateError::PaymentIntentMismatch { .. } => {
                            "Refresh checkout before placing the order."
                        }
                        _ => "There is a problem with your checkout details.",
                    };
                    let mut form_state =
                        storefront_checkout_form_state_from_execution(execution, summary);
                    if matches!(
                        error,
                        StorefrontStateError::MissingPaymentIntent
                            | StorefrontStateError::PaymentIntentMismatch { .. }
                    ) {
                        form_state = form_state.with_field_error(
                            "checkout_intent",
                            "Refresh checkout and try again before placing the order.",
                        );
                    }
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/checkout".to_string()));
                }
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            };
            if let Some(location) = finalize_storefront_checkout_completion(
                state,
                execution,
                &snapshot,
                now,
                response_cookies,
            )
            .await?
            {
                return Ok(Some(location));
            }
        }
        "commerce.catalog-admin-update" => {
            let update = match validated_catalog_admin_update_from_execution(execution) {
                Ok(update) => update,
                Err(form_state) => {
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/admin/catalog/products".to_string()));
                }
            };
            let update_result = match &update {
                CatalogAdminMutationInput::Product(update) => state
                    .storefront
                    .update_catalog_product(update, now.as_unix_seconds()),
                CatalogAdminMutationInput::Collection(update) => state
                    .storefront
                    .update_catalog_collection(update, now.as_unix_seconds()),
            };
            match update_result {
                Ok(_) => {
                    let message = match &update {
                        CatalogAdminMutationInput::Product(update) => {
                            format!("Saved product changes for {}.", update.title)
                        }
                        CatalogAdminMutationInput::Collection(update) => {
                            format!("Saved collection changes for {}.", update.title)
                        }
                    };
                    match &update {
                        CatalogAdminMutationInput::Product(update) => {
                            record_operator_audit(
                                state,
                                execution,
                                "commerce.catalog-admin-update.product",
                                "product",
                                update.handle.as_str(),
                                "succeeded",
                                &format!(
                                    "Updated product `{}` at price_minor {} and marked it {}.",
                                    update.title,
                                    update.price_minor,
                                    if update.is_visible {
                                        "visible"
                                    } else {
                                        "hidden"
                                    }
                                ),
                            )?;
                        }
                        CatalogAdminMutationInput::Collection(update) => {
                            record_operator_audit(
                                state,
                                execution,
                                "commerce.catalog-admin-update.collection",
                                "collection",
                                update.handle.as_str(),
                                "succeeded",
                                &format!(
                                    "Updated collection `{}` and marked it {}.",
                                    update.title,
                                    if update.is_visible {
                                        "visible"
                                    } else {
                                        "hidden"
                                    }
                                ),
                            )?;
                        }
                    }
                    push_storefront_flash(state, response_cookies, FlashLevel::Success, message)?;
                    return Ok(Some("/admin/catalog/products".to_string()));
                }
                Err(
                    error @ (StorefrontStateError::MissingCatalogProduct { .. }
                    | StorefrontStateError::MissingCatalogCollection { .. }),
                ) => {
                    let mut form_state = match &update {
                        CatalogAdminMutationInput::Product(_) => {
                            catalog_admin_product_form_state_from_execution(
                                execution,
                                "Refresh the catalog admin page and try again.",
                            )
                        }
                        CatalogAdminMutationInput::Collection(_) => {
                            catalog_admin_collection_form_state_from_execution(
                                execution,
                                "Refresh the catalog admin page and try again.",
                            )
                        }
                    };
                    form_state = form_state.with_summary(error.to_string());
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/admin/catalog/products".to_string()));
                }
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            }
        }
        "commerce.order-refund" => {
            let order_id = storefront_form_field_value(execution, "order_id");
            let reason = storefront_form_field_value(execution, "reason");
            let redirect_location = if order_id.trim().is_empty() {
                "/admin/orders".to_string()
            } else {
                format!("/admin/orders/{}", order_id.trim())
            };
            match state.storefront.refund_order(
                order_id.trim(),
                reason.as_str(),
                now.as_unix_seconds(),
            ) {
                Ok(order) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-refund",
                        "order",
                        order.order_id.as_str(),
                        "succeeded",
                        &format!(
                            "Refunded {} with reason `{}`.",
                            order.refunded_total, reason
                        ),
                    )?;
                    push_storefront_flash(
                        state,
                        response_cookies,
                        FlashLevel::Success,
                        format!(
                            "Refunded {} for order {}.",
                            order.refunded_total, order.order_id
                        ),
                    )?;
                    return Ok(Some(format!("/admin/orders/{}", order.order_id)));
                }
                Err(StorefrontStateError::MissingRefundReason) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-refund",
                        "order",
                        order_id.trim(),
                        "rejected",
                        "Refund reason was required but not provided.",
                    )?;
                    let form_state = order_refund_form_state_from_execution(
                        execution,
                        "Review the refund request and add a reason before trying again.",
                    )
                    .with_field_error("reason", "refund reason is required");
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some(redirect_location));
                }
                Err(error @ StorefrontStateError::RefundNotAllowed { .. }) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-refund",
                        "order",
                        order_id.trim(),
                        "rejected",
                        &error.to_string(),
                    )?;
                    let form_state = order_refund_form_state_from_execution(
                        execution,
                        "This order cannot be refunded from the checked-in admin workflow right now.",
                    )
                    .with_field_error("reason", error.to_string());
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some(redirect_location));
                }
                Err(error @ StorefrontStateError::UnknownOrder { .. }) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-refund",
                        "order",
                        order_id.trim(),
                        "rejected",
                        &error.to_string(),
                    )?;
                    if order_id.trim().is_empty() {
                        push_storefront_flash(
                            state,
                            response_cookies,
                            FlashLevel::Error,
                            error.to_string(),
                        )?;
                    } else {
                        let form_state = order_refund_form_state_from_execution(
                            execution,
                            "Refresh the order queue and reopen the detail view before retrying this refund.",
                        )
                        .with_field_error("order_id", error.to_string());
                        push_storefront_form_state(state, response_cookies, &form_state)?;
                    }
                    return Ok(Some(redirect_location));
                }
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            }
        }
        "commerce.order-fulfill" => {
            let order_id = storefront_form_field_value(execution, "order_id");
            let redirect_location = if order_id.trim().is_empty() {
                "/admin/orders".to_string()
            } else {
                format!("/admin/orders/{}", order_id.trim())
            };
            match state
                .storefront
                .fulfill_order(order_id.trim(), now.as_unix_seconds())
            {
                Ok(order) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-fulfill",
                        "order",
                        order.order_id.as_str(),
                        "succeeded",
                        &format!("Marked {} as fulfilled.", order.order_id),
                    )?;
                    push_storefront_flash(
                        state,
                        response_cookies,
                        FlashLevel::Success,
                        format!("Marked order {} as fulfilled.", order.order_id),
                    )?;
                    return Ok(Some(format!("/admin/orders/{}", order.order_id)));
                }
                Err(StorefrontStateError::FulfillmentNotAllowed { order_id, status }) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-fulfill",
                        "order",
                        order_id.as_str(),
                        "rejected",
                        &format!("Order cannot be fulfilled while it is `{status}`."),
                    )?;
                    push_storefront_flash(
                        state,
                        response_cookies,
                        FlashLevel::Error,
                        format!(
                            "Order {} cannot be marked fulfilled while it is {}.",
                            order_id, status
                        ),
                    )?;
                    return Ok(Some(redirect_location));
                }
                Err(error @ StorefrontStateError::UnknownOrder { .. }) => {
                    record_operator_audit(
                        state,
                        execution,
                        "commerce.order-fulfill",
                        "order",
                        order_id.trim(),
                        "rejected",
                        &error.to_string(),
                    )?;
                    if order_id.trim().is_empty() {
                        push_storefront_flash(
                            state,
                            response_cookies,
                            FlashLevel::Error,
                            error.to_string(),
                        )?;
                    } else {
                        let form_state = StorefrontFormState::new(
                            "commerce.orders",
                            "Refresh the order queue and reopen the detail view before retrying this fulfillment.",
                        );
                        push_storefront_form_state(state, response_cookies, &form_state)?;
                    }
                    return Ok(Some(redirect_location));
                }
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            }
        }
        "commerce.account-session-end" => {
            revoke_storefront_session(state, session_id, now, response_cookies)?;
            push_storefront_flash(
                state,
                response_cookies,
                FlashLevel::Success,
                "Account session ended. Start again from this browser when you are ready.",
            )?;
            return Ok(Some("/account".to_string()));
        }
        _ => {}
    }
    Ok(None)
}

async fn finalize_storefront_checkout_completion(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    snapshot: &StorefrontStateSnapshot,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    let Some(order) = snapshot.latest_order.as_ref() else {
        push_storefront_flash(
            state,
            response_cookies,
            FlashLevel::Error,
            "Checkout could not complete because the cart is empty.",
        )?;
        return Ok(None);
    };

    let Some(provider) = configured_commerce_payment_provider(&state.plan.config) else {
        push_storefront_flash(
            state,
            response_cookies,
            FlashLevel::Success,
            format!(
                "Order {} was received. Payment is still awaiting provider confirmation.",
                order.order_id
            ),
        )?;
        return Ok(None);
    };

    if provider.code == "stripe" && provider.uses_hosted_checkout() {
        match launch_stripe_checkout_handoff(state, execution, order).await {
            Ok(handoff_url) => return Ok(Some(handoff_url)),
            Err(_) => {
                return restore_checkout_after_provider_handoff_failure(
                    state,
                    order,
                    now,
                    response_cookies,
                    "Stripe checkout could not start. Your basket has been restored so you can review it and try again.",
                )
                .map(Some);
            }
        }
    }

    push_storefront_flash(
        state,
        response_cookies,
        FlashLevel::Success,
        provider.pending_confirmation_summary(&order.order_id),
    )?;
    Ok(None)
}

async fn launch_stripe_checkout_handoff(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    order: &StorefrontOrderSnapshot,
) -> Result<String, String> {
    let payment_reference = order
        .payment
        .reference
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("order {} is missing a payment reference", order.order_id))?;
    if state.uses_development_hosted_checkout_stub() {
        return Ok(provider_checkout_return_url(
            execution,
            payment_reference,
            "return",
        ));
    }
    let api_key = state
        .payment_provider_api_key
        .as_deref()
        .ok_or_else(|| "stripe hosted checkout api key is not configured".to_string())?
        .to_string();
    let request_body = stripe_checkout_session_request_body(execution, order)?;
    let idempotency_key = format!("davenda-order-{}", order.order_id);
    let checkout_client = Arc::clone(&state.hosted_checkout_client);
    let response = tokio::task::spawn_blocking(move || {
        checkout_client.create_stripe_checkout_session(&api_key, &request_body, &idempotency_key)
    })
    .await
    .map_err(|error| format!("failed to join Stripe Checkout handoff task: {error}"))??;

    if response.id.trim().is_empty() || response.url.trim().is_empty() {
        return Err("Stripe Checkout response was missing the hosted session URL".to_string());
    }
    Ok(response.url)
}

fn stripe_checkout_session_request_body(
    execution: &RequestExecution,
    order: &StorefrontOrderSnapshot,
) -> Result<String, String> {
    let payment_reference = order
        .payment
        .reference
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("order {} is missing a payment reference", order.order_id))?;
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("mode", "payment");
    serializer.append_pair(
        "success_url",
        &provider_checkout_return_url(execution, payment_reference, "return"),
    );
    serializer.append_pair(
        "cancel_url",
        &provider_checkout_return_url(execution, payment_reference, "cancel"),
    );
    serializer.append_pair("client_reference_id", payment_reference);
    if let Some(email) = order.payment.checkout_email.as_deref() {
        let trimmed = email.trim();
        if !trimmed.is_empty() {
            serializer.append_pair("customer_email", trimmed);
        }
    }
    serializer.append_pair("payment_intent_data[metadata][order_id]", &order.order_id);
    serializer.append_pair(
        "payment_intent_data[metadata][payment_reference]",
        payment_reference,
    );
    serializer.append_pair("metadata[order_id]", &order.order_id);
    serializer.append_pair("metadata[payment_reference]", payment_reference);

    for (index, line) in order.lines.iter().enumerate() {
        if line.quantity == 0 {
            return Err(format!(
                "order {} contains a zero-quantity line for {}",
                order.order_id, line.sku
            ));
        }
        if line.unit_price_minor <= 0 {
            return Err(format!(
                "order {} contains a non-positive unit amount for {}",
                order.order_id, line.sku
            ));
        }

        let prefix = format!("line_items[{index}]");
        serializer.append_pair(
            &format!("{prefix}[price_data][currency]"),
            &line.currency.to_ascii_lowercase(),
        );
        serializer.append_pair(
            &format!("{prefix}[price_data][unit_amount]"),
            &line.unit_price_minor.to_string(),
        );
        serializer.append_pair(
            &format!("{prefix}[price_data][product_data][name]"),
            stripe_line_item_name(line).as_str(),
        );
        serializer.append_pair(&format!("{prefix}[quantity]"), &line.quantity.to_string());
    }

    Ok(serializer.finish())
}

fn stripe_line_item_name(line: &StorefrontOrderLine) -> String {
    let variant = line.variant_title.trim();
    if variant.is_empty() || variant.eq_ignore_ascii_case("standard") {
        return line.title.clone();
    }
    format!("{} ({variant})", line.title)
}

fn provider_checkout_return_url(
    execution: &RequestExecution,
    payment_reference: &str,
    provider_result: &str,
) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("provider_result", provider_result);
    serializer.append_pair("payment_reference", payment_reference);
    format!(
        "{}://{}/checkout/confirmation?{}",
        execution.trace.transport_scheme,
        execution.host,
        serializer.finish()
    )
}

fn restore_checkout_after_provider_handoff_failure(
    state: &RuntimeServerState,
    order: &StorefrontOrderSnapshot,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
    message: &str,
) -> Result<String, RuntimeServerError> {
    if let Some(payment_reference) = order.payment.reference.as_deref() {
        let _ = state.storefront.apply_payment_webhook(
            payment_reference,
            "payment.failed",
            now.as_unix_seconds(),
        )?;
    }
    push_storefront_flash(state, response_cookies, FlashLevel::Error, message)?;
    Ok("/cart".to_string())
}

fn redirect_failed_checkout_confirmation(
    state: &RuntimeServerState,
    route_name: &str,
    method: HttpMethod,
    session_id: Option<&str>,
    principal_id: Option<&str>,
    provider_result: Option<&str>,
    payment_reference: Option<&str>,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if route_name != "commerce.checkout-confirmation" || method != HttpMethod::Get {
        return Ok(None);
    }
    if provider_result == Some("return")
        && state.uses_development_hosted_checkout_stub()
        && let Some(payment_reference) = payment_reference
    {
        let receipt = state.storefront.apply_payment_webhook(
            payment_reference,
            "payment.succeeded",
            now.as_unix_seconds(),
        )?;
        dispatch_paid_order_event(state, &receipt.order, now)?;
        push_storefront_flash(
            state,
            response_cookies,
            FlashLevel::Success,
            format!(
                "Local checkout completed for order {} using the built-in development payment stub.",
                receipt.order.order_id
            ),
        )?;
        return Ok(None);
    }
    if provider_result == Some("cancel") {
        if let Some(payment_reference) = payment_reference {
            match state.storefront.apply_payment_webhook(
                payment_reference,
                "payment.failed",
                now.as_unix_seconds(),
            ) {
                Ok(receipt) => {
                    if receipt.order.payment.status == "failed" {
                        push_storefront_flash(
                            state,
                            response_cookies,
                            FlashLevel::Error,
                            "Stripe checkout was cancelled. Your basket has been restored so you can review it and start checkout again.",
                        )?;
                        return Ok(Some("/cart".to_string()));
                    }
                }
                Err(StorefrontStateError::UnknownPaymentReference { .. }) => {}
                Err(error) => return Err(RuntimeServerError::Storefront(error)),
            }
        }
    }
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    let snapshot = state.storefront.snapshot(session_id, principal_id)?;
    let Some(order) = snapshot.latest_order.as_ref() else {
        return Ok(None);
    };
    if order.payment.status != "failed" {
        return Ok(None);
    }
    push_storefront_flash(
        state,
        response_cookies,
        FlashLevel::Error,
        format!(
            "Payment for order {} failed. Your basket has been restored so you can review it and start checkout again.",
            order.order_id
        ),
    )?;
    Ok(Some("/cart".to_string()))
}

fn dispatch_paid_order_event(
    state: &RuntimeServerState,
    order: &StorefrontOrderSnapshot,
    now: BrowserInstant,
) -> Result<(), RuntimeServerError> {
    let mut jobs = state.plan.jobs_host("runtime-http")?;
    let payment_reference = order
        .payment
        .reference
        .as_deref()
        .unwrap_or(order.order_id.as_str());
    let _ = jobs.emit_domain_event(
        DomainEventDispatchRequest::new(
            "commerce.order.paid",
            "order",
            order.order_id.clone(),
            format!("payment provider confirmed {payment_reference}"),
        )?,
        JobInstant::from_unix_seconds(now.as_unix_seconds()),
    )?;
    Ok(())
}

fn execution_form_field<'a>(execution: &'a RequestExecution, name: &str) -> Option<&'a str> {
    execution
        .form_fields
        .get(name)
        .and_then(|values| values.first().map(String::as_str))
}

fn execution_query_field<'a>(execution: &'a RequestExecution, name: &str) -> Option<&'a str> {
    execution
        .query_params
        .get(name)
        .and_then(|values| values.first().map(String::as_str))
}

fn run_customer_hook_future<T>(
    future: impl Future<Output = Result<T, RuntimeServerError>> + Send + 'static,
) -> Result<T, RuntimeServerError>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), tokio::runtime::RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Ok(_) => std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| RuntimeServerError::CustomerHookFailed {
                    surface: "auth",
                    reason: format!("failed to build runtime bridge for customer hooks: {error}"),
                })?
                .block_on(future)
        })
        .join()
        .map_err(|_| RuntimeServerError::CustomerHookFailed {
            surface: "auth",
            reason: "customer hook runtime bridge thread panicked".to_string(),
        })?,
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| RuntimeServerError::CustomerHookFailed {
                surface: "auth",
                reason: format!("failed to build runtime bridge for customer hooks: {error}"),
            })?
            .block_on(future),
    }
}

fn customer_hook_auth_backend_error(error: RuntimeServerError) -> BackendError {
    BackendError::new(
        BackendErrorKind::Unavailable,
        "auth.live_check.failed",
        "Runtime could not complete the linked customer auth check.",
    )
    .with_detail(error.to_string())
}

fn parse_customer_capability(value: &str) -> Result<davenda_auth::Capability, BackendError> {
    davenda_auth::Capability::from_str(value).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.capability.invalid",
            format!("Unknown capability `{value}`."),
        )
    })
}

fn parse_customer_auth_entity(value: &str) -> Result<davenda_auth::Entity, BackendError> {
    let Some((namespace, id)) = value.split_once(':') else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    };
    if id.trim().is_empty() {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    }
    match namespace {
        "tenant" => Ok(davenda_auth::Entity::tenant(id)),
        "site" => Ok(davenda_auth::Entity::site(id)),
        "brand" => Ok(davenda_auth::Entity::brand(id)),
        "storefront" => Ok(davenda_auth::Entity::storefront(id)),
        "user" => Ok(davenda_auth::Entity::user(id)),
        "group" => Ok(davenda_auth::Entity::group(id)),
        "team" => Ok(davenda_auth::Entity::team(id)),
        "service_account" => Ok(davenda_auth::Entity::service_account(id)),
        "page" => Ok(davenda_auth::Entity::page(id)),
        "navigation" => Ok(davenda_auth::Entity::navigation(id)),
        "product" => Ok(davenda_auth::Entity::product(id)),
        "collection" => Ok(davenda_auth::Entity::collection(id)),
        "order" => Ok(davenda_auth::Entity::order(id)),
        "subscription" => Ok(davenda_auth::Entity::subscription(id)),
        "membership_tier" => Ok(davenda_auth::Entity::membership_tier(id)),
        "event" => Ok(davenda_auth::Entity::event(id)),
        "event_slot" => Ok(davenda_auth::Entity::event_slot(id)),
        "booking" => Ok(davenda_auth::Entity::booking(id)),
        "media" => Ok(davenda_auth::Entity::media(id)),
        "media_library" => Ok(davenda_auth::Entity::media_library(id)),
        "asset" => Ok(davenda_auth::Entity::asset(id)),
        "asset_folder" => Ok(davenda_auth::Entity::asset_folder(id)),
        "theme_asset_bundle" => Ok(davenda_auth::Entity::theme_asset_bundle(id)),
        "admin_module" => Ok(davenda_auth::Entity::admin_module(id)),
        _ => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Unknown auth object namespace `{namespace}`."),
        )),
    }
}

fn customer_hook_auth_subject(principal_id: Option<&str>) -> davenda_auth::DefaultSubject {
    match principal_id {
        Some(principal_id) => {
            davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(principal_id.to_string()))
        }
        None => davenda_auth::DefaultSubject::entity(davenda_auth::Entity::any_user()),
    }
}

fn decode_hex_signature(signature: &str) -> Option<Vec<u8>> {
    if !signature.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(signature.len() / 2);
    let mut chars = signature.as_bytes().chunks_exact(2);
    for chunk in &mut chars {
        let high = decode_hex_nibble(chunk[0])?;
        let low = decode_hex_nibble(chunk[1])?;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn storefront_sku_from_execution(
    execution: &RequestExecution,
) -> Result<Cow<'_, str>, RuntimeServerError> {
    execution_form_field(execution, "sku")
        .or_else(|| execution_form_field(execution, "product_slug"))
        .or_else(|| execution_form_field(execution, "line_id"))
        .map(Cow::Borrowed)
        .ok_or_else(|| {
            RuntimeServerError::Storefront(StorefrontStateError::UnknownSku {
                sku: "<missing>".to_string(),
            })
        })
}

fn cart_quantities_from_execution(execution: &RequestExecution) -> BTreeMap<String, u32> {
    let mut quantities = BTreeMap::new();
    if let Ok(sku) = storefront_sku_from_execution(execution) {
        quantities.insert(
            sku.into_owned(),
            parse_quantity_field(execution_form_field(execution, "quantity")).unwrap_or(1),
        );
    }
    for (name, values) in &execution.form_fields {
        let Some(product_slug) = name.strip_prefix("quantity_") else {
            continue;
        };
        let Some(quantity) = values
            .first()
            .and_then(|value| parse_quantity_field(Some(value.as_str())))
        else {
            continue;
        };
        quantities.insert(product_slug.to_string(), quantity);
    }
    quantities
}

fn storefront_response_augmentation(
    state: &RuntimeServerState,
    execution: &RequestExecution,
) -> Result<Option<StorefrontResponseAugmentation>, RuntimeServerError> {
    let should_render_storefront = should_render_storefront_state(execution);
    let should_render_cms_admin_forms = should_render_cms_admin_forms(execution);
    if !should_render_storefront && !should_render_cms_admin_forms {
        return Ok(None);
    }
    let Some(session_id) = execution.session.session_id.as_deref() else {
        return Ok(None);
    };
    let mut augmentation = if should_render_storefront {
        let snapshot = state
            .storefront
            .snapshot(session_id, execution.principal.principal_id.as_deref())?;
        let tokens = issue_storefront_csrf_tokens(state, session_id)?;
        state.storefront.build_response_augmentation(
            execution.route.route_name.as_str(),
            &snapshot,
            tokens,
        )?
    } else {
        StorefrontResponseAugmentation {
            html_fragment: None,
            headers: BTreeMap::new(),
        }
    };
    if should_render_cms_admin_forms {
        augmentation
            .headers
            .extend(issue_cms_admin_csrf_tokens(state, session_id)?);
    }
    Ok(Some(augmentation))
}

fn should_render_storefront_state(execution: &RequestExecution) -> bool {
    matches!(execution.response, HandlerResponse::Page(_))
        && (execution.route.route_name.starts_with("commerce.")
            || execution.route_area == RouteArea::Account)
}

fn should_render_cms_admin_forms(execution: &RequestExecution) -> bool {
    matches!(execution.response, HandlerResponse::Page(_))
        && matches!(
            execution.route.route_name.as_str(),
            "cms.pages.index" | "cms.navigation.index" | "cms.redirects.index"
        )
}

fn issue_storefront_csrf_tokens(
    state: &RuntimeServerState,
    session_id: &str,
) -> Result<BTreeMap<String, String>, RuntimeServerError> {
    let browser = state
        .browser
        .lock()
        .expect("runtime browser mutex poisoned");
    let mut tokens = BTreeMap::new();
    for action in STOREFRONT_CSRF_ACTIONS {
        let token = browser
            .issue_csrf_token(&state.csrf_secret, session_id, action)
            .map_err(RequestExecutionError::from_browser_error)?;
        tokens.insert((*action).to_string(), token);
    }
    Ok(tokens)
}

fn issue_cms_admin_csrf_tokens(
    state: &RuntimeServerState,
    session_id: &str,
) -> Result<BTreeMap<String, String>, RuntimeServerError> {
    let browser = state
        .browser
        .lock()
        .expect("runtime browser mutex poisoned");
    let mut tokens = BTreeMap::new();
    for (action, header) in CMS_ADMIN_CSRF_ACTIONS {
        let token = browser
            .issue_csrf_token(&state.csrf_secret, session_id, action)
            .map_err(RequestExecutionError::from_browser_error)?;
        tokens.insert((*header).to_string(), token);
    }
    Ok(tokens)
}

fn storefront_order_history_response(
    state: &RuntimeServerState,
    request: &RequestInput,
    response_cookies: Vec<String>,
) -> Result<Response<Body>, RuntimeServerError> {
    let Some(session_id) = request.session_id.as_deref() else {
        return Err(RuntimeServerError::Execution(
            RequestExecutionError::SessionRequired {
                route: "account.orders".to_string(),
            },
        ));
    };
    let history =
        state
            .storefront
            .order_history(session_id, request.principal_id.as_deref(), 50)?;
    let body = serde_json::to_string(&history).map_err(|error| {
        RuntimeServerError::Storefront(StorefrontStateError::Serialization {
            reason: error.to_string(),
        })
    })?;
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-davenda-storefront-order-count"),
        HeaderValue::from_str(&history.orders.len().to_string())
            .expect("order count is a valid header value"),
    );
    if let Some(order) = history.orders.first() {
        response.headers_mut().insert(
            HeaderName::from_static("x-davenda-storefront-latest-order"),
            HeaderValue::from_str(order.order_id.as_str())
                .expect("order id is a valid header value"),
        );
    }
    for cookie in response_cookies {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response
                .headers_mut()
                .append(HeaderName::from_static("set-cookie"), value);
        }
    }
    Ok(response)
}

async fn apply_storefront_response_augmentation(
    mut response: Response<Body>,
    augmentation: Option<StorefrontResponseAugmentation>,
) -> Result<Response<Body>, RuntimeServerError> {
    let Some(augmentation) = augmentation else {
        return Ok(response);
    };
    let form_tokens = storefront_form_tokens_from_headers(&augmentation.headers);
    for (name, value) in augmentation.headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            response.headers_mut().insert(name, value);
        }
    }
    let Some(markup) = augmentation.html_fragment else {
        return Ok(response);
    };
    let is_html = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if !is_html {
        return Ok(response);
    }
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| RuntimeServerError::RequestBodyTooLarge { limit: usize::MAX })?;
    let html = String::from_utf8(bytes.to_vec()).map_err(|error| {
        RuntimeServerError::Storefront(StorefrontStateError::Serialization {
            reason: error.to_string(),
        })
    })?;
    let html = inject_storefront_form_csrf_inputs(html, form_tokens.as_slice());
    Ok(Response::from_parts(
        parts,
        Body::from(inject_storefront_markup(html, markup.as_str())),
    ))
}

fn inject_storefront_markup(document_html: String, markup: &str) -> String {
    if markup.is_empty() {
        return document_html;
    }
    if let Some(index) = document_html.find("</body>") {
        let mut html = document_html;
        html.insert_str(index, markup);
        return html;
    }
    format!("{document_html}{markup}")
}

fn storefront_form_tokens_from_headers(
    headers: &BTreeMap<String, String>,
) -> Vec<(&'static str, String)> {
    STOREFRONT_FORM_CSRF_HEADERS
        .iter()
        .chain(CMS_ADMIN_FORM_CSRF_HEADERS.iter())
        .filter_map(|(path, header)| headers.get(*header).map(|token| (*path, token.clone())))
        .collect()
}

fn inject_storefront_form_csrf_inputs(
    mut document_html: String,
    form_tokens: &[(&'static str, String)],
) -> String {
    for (action_path, token) in form_tokens {
        document_html = inject_hidden_csrf_input(document_html, action_path, token.as_str());
    }
    document_html
}

fn inject_hidden_csrf_input(mut document_html: String, action_path: &str, token: &str) -> String {
    let action_attr = format!("action=\"{action_path}\"");
    let hidden_input = format!(r#"<input type="hidden" name="_csrf" value="{token}" />"#);
    let mut search_from = 0;

    while let Some(relative) = document_html[search_from..].find(&action_attr) {
        let action_index = search_from + relative;
        let Some(form_start) = document_html[..action_index].rfind("<form") else {
            search_from = action_index + action_attr.len();
            continue;
        };
        let Some(open_end_relative) = document_html[action_index..].find('>') else {
            break;
        };
        let open_end = action_index + open_end_relative;
        let Some(close_relative) = document_html[open_end..].find("</form>") else {
            break;
        };
        let close_index = open_end + close_relative;
        if document_html[open_end..close_index].contains("name=\"_csrf\"") {
            search_from = close_index + "</form>".len();
            continue;
        }
        document_html.insert_str(open_end + 1, hidden_input.as_str());
        search_from = open_end + 1 + hidden_input.len();
        if search_from < form_start {
            search_from = form_start;
        }
    }

    document_html
}

async fn enforce_request_body_limit(
    request: Request<Body>,
    max_body_bytes: Option<usize>,
) -> Result<Request<Body>, RuntimeServerError> {
    let Some(limit) = max_body_bytes else {
        return Ok(request);
    };

    let (parts, body) = request.into_parts();
    if let Some(content_length) = parts
        .headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        && content_length > limit
    {
        return Err(RuntimeServerError::RequestBodyTooLarge { limit });
    }

    let bytes = to_bytes(body, limit)
        .await
        .map_err(|_| RuntimeServerError::RequestBodyTooLarge { limit })?;
    Ok(Request::from_parts(parts, Body::from(bytes)))
}

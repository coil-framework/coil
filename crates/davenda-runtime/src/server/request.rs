use super::auth::authorize_live_request;
use super::*;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HOST};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

const STOREFRONT_ORDER_HISTORY_JSON_PATH: &str = "/account/orders.json";
const STOREFRONT_NATIVE_CAPABILITY_ROUTES: &[&str] = &[
    "commerce.cart",
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout",
    "commerce.checkout-start",
    "commerce.checkout-complete",
    "commerce.checkout-confirmation",
    "commerce.account-session-end",
];
const STOREFRONT_CSRF_ACTIONS: &[&str] = &[
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout-start",
    "commerce.checkout-complete",
    "commerce.account-session-end",
];

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifiedPaymentWebhook {
    event: String,
    payment_reference: String,
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
        apply_native_storefront_mutations(state, &execution, now, &mut storefront_mutation_cookies)?
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
    if let Some(location) = redirect_failed_checkout_confirmation(
        state,
        route_name.as_str(),
        method,
        session_id.as_deref(),
        principal_id.as_deref(),
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
    execution: &RequestExecution,
) -> Result<StorefrontPaymentInput, StorefrontFormState> {
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
    if method.is_empty() {
        has_errors = true;
        form_state = form_state.with_field_error(
            "payment_method",
            "Choose or confirm a payment method before placing the order.",
        );
    }
    if method == "card"
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
    StorefrontPaymentInput::new(
        method,
        checkout_email,
        (!last4.is_empty()).then_some(last4),
        intent_reference,
    )
    .map_err(|error| {
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

fn apply_native_storefront_mutations(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if execution.route.route_name.as_str() == "commerce.payment-provider-webhook" {
        let webhook = validated_payment_webhook_from_execution(state, execution)?;
        let receipt = state.storefront.apply_payment_webhook(
            webhook.payment_reference.as_str(),
            webhook.event.as_str(),
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
            let payment = match validated_storefront_payment_input_from_execution(execution) {
                Ok(payment) => payment,
                Err(form_state) => {
                    push_storefront_form_state(state, response_cookies, &form_state)?;
                    return Ok(Some("/checkout".to_string()));
                }
            };
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
            let message = snapshot
                .latest_order
                .as_ref()
                .map(|order| {
                    format!(
                        "Order {} was received. Payment is still awaiting provider confirmation.",
                        order.order_id
                    )
                })
                .unwrap_or_else(|| {
                    "Checkout could not complete because the cart is empty.".to_string()
                });
            push_storefront_flash(state, response_cookies, FlashLevel::Success, message)?;
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

fn redirect_failed_checkout_confirmation(
    state: &RuntimeServerState,
    route_name: &str,
    method: HttpMethod,
    session_id: Option<&str>,
    principal_id: Option<&str>,
    response_cookies: &mut Vec<String>,
) -> Result<Option<String>, RuntimeServerError> {
    if route_name != "commerce.checkout-confirmation" || method != HttpMethod::Get {
        return Ok(None);
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
    if !should_render_storefront_state(execution) {
        return Ok(None);
    }
    let Some(session_id) = execution.session.session_id.as_deref() else {
        return Ok(None);
    };
    let snapshot = state
        .storefront
        .snapshot(session_id, execution.principal.principal_id.as_deref())?;
    let tokens = issue_storefront_csrf_tokens(state, session_id)?;
    Ok(Some(state.storefront.build_response_augmentation(
        execution.route.route_name.as_str(),
        &snapshot,
        tokens,
    )?))
}

fn should_render_storefront_state(execution: &RequestExecution) -> bool {
    matches!(execution.response, HandlerResponse::Page(_))
        && (execution.route.route_name.starts_with("commerce.")
            || execution.route_area == RouteArea::Account)
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

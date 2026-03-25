use super::auth::authorize_live_request;
use super::*;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HOST};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

const STOREFRONT_ORDER_HISTORY_PATH: &str = "/account/orders";
const STOREFRONT_NATIVE_CAPABILITY_ROUTES: &[&str] = &[
    "commerce.cart",
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout",
    "commerce.checkout-start",
    "commerce.checkout-complete",
    "commerce.checkout-confirmation",
];
const STOREFRONT_CSRF_ACTIONS: &[&str] = &[
    "commerce.add-to-cart",
    "commerce.cart-update",
    "commerce.checkout-start",
    "commerce.checkout-complete",
];

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
        if request.path == STOREFRONT_ORDER_HISTORY_PATH && request.method == HttpMethod::Get {
            return storefront_order_history_response(state, &request, native_response_cookies);
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
        execution.response_cookies = native_response_cookies.clone();
        execution
    } else {
        prepare_native_storefront_request(state, &mut request, now, &mut native_response_cookies)?;
        if request.path == STOREFRONT_ORDER_HISTORY_PATH && request.method == HttpMethod::Get {
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
    apply_native_storefront_mutations(state, &execution, now, &mut storefront_mutation_cookies)?;
    execution
        .response_cookies
        .extend(storefront_mutation_cookies);
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
            | StorefrontStateError::EmptyCart { .. },
        ) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
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
    let is_native_capability_route = STOREFRONT_NATIVE_CAPABILITY_ROUTES.contains(&route_name);
    let is_native_mutation_route = matches!(
        route_name,
        "commerce.add-to-cart"
            | "commerce.cart-update"
            | "commerce.checkout-start"
            | "commerce.checkout-complete"
    );
    if !is_storefront_page && !is_native_mutation_route {
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

fn parse_quantity_field(value: Option<&str>) -> Option<u32> {
    value.and_then(|raw| raw.trim().parse::<u32>().ok())
}

fn apply_native_storefront_mutations(
    state: &RuntimeServerState,
    execution: &RequestExecution,
    now: BrowserInstant,
    response_cookies: &mut Vec<String>,
) -> Result<(), RuntimeServerError> {
    let Some(session_id) = execution.session.session_id.as_deref() else {
        return Ok(());
    };
    match execution.route.route_name.as_str() {
        "commerce.add-to-cart" => {
            let quantity =
                parse_quantity_field(execution_form_field(execution, "quantity")).unwrap_or(1);
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
            let quantities = cart_quantities_from_execution(execution);
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
            let _ = state.storefront.checkout_start(
                session_id,
                execution.principal.principal_id.as_deref(),
                now.as_unix_seconds(),
            )?;
        }
        "commerce.checkout-complete" => {
            let snapshot = state.storefront.checkout_complete(
                session_id,
                execution.principal.principal_id.as_deref(),
                now.as_unix_seconds(),
            )?;
            let message = snapshot
                .latest_order
                .as_ref()
                .map(|order| format!("Order {} is confirmed.", order.order_id))
                .unwrap_or_else(|| {
                    "Checkout could not complete because the cart is empty.".to_string()
                });
            push_storefront_flash(state, response_cookies, FlashLevel::Success, message)?;
        }
        _ => {}
    }
    Ok(())
}

fn execution_form_field<'a>(execution: &'a RequestExecution, name: &str) -> Option<&'a str> {
    execution
        .form_fields
        .get(name)
        .and_then(|values| values.first().map(String::as_str))
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

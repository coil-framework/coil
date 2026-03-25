use super::auth::authorize_live_request;
use super::*;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HOST};
use axum::http::{HeaderMap, Method, Request, Response, StatusCode};
use axum::response::IntoResponse;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

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

    execution_response(&state.plan, &state.wasm_host, execution)
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

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use davenda_customer_sdk::{OutboundHttpRequest, OutboundHttpResponse};
use davenda_wasm::NetworkExecution;
use ureq::{Agent, AgentBuilder};
use url::Url;

use super::super::super::*;
use super::offload::offload_outbound_http_to_blocking_pool;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES_FROM_HINT: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeOutboundHttpBackend {
    allow_network: bool,
    targets: Arc<BTreeMap<String, Url>>,
    client: Agent,
    request_timeout: Duration,
    max_response_bytes: u64,
}

impl RuntimeOutboundHttpBackend {
    pub(crate) fn with_targets(allow_network: bool, targets: BTreeMap<String, Url>) -> Self {
        Self::with_settings(
            allow_network,
            targets,
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_MAX_RESPONSE_BYTES,
        )
    }

    pub(crate) fn with_settings(
        allow_network: bool,
        targets: BTreeMap<String, Url>,
        request_timeout: Duration,
        max_response_bytes: u64,
    ) -> Self {
        Self {
            allow_network,
            targets: Arc::new(targets),
            client: AgentBuilder::new()
                .timeout_connect(DEFAULT_CONNECT_TIMEOUT)
                .timeout_read(request_timeout)
                .build(),
            request_timeout,
            max_response_bytes: max_response_bytes.max(1),
        }
    }

    /// Submit an approved outbound HTTP integration to the blocking pool.
    ///
    /// This returns only after the offloaded request completes, but the actual
    /// network call and response body read do not run on the core worker lane.
    pub(crate) fn submit_outbound_http_to_blocking_pool(
        &self,
        integration: &str,
        response_bytes_hint: u64,
        _context: &InvocationContext,
    ) -> Result<NetworkExecution, String> {
        if !self.allow_network {
            return Err("outbound network is disabled for this runtime".to_string());
        }

        let endpoint = self.resolve_endpoint(integration)?;
        let byte_limit = self.response_byte_limit(response_bytes_hint)?;
        let integration = integration.to_string();
        let endpoint_string = endpoint.to_string();
        let client = self.client.clone();
        let request_timeout = self.request_timeout;
        offload_outbound_http_to_blocking_pool(move || {
            perform_request(
                client,
                endpoint,
                endpoint_string,
                integration,
                request_timeout,
                byte_limit,
            )
        })
    }

    pub(crate) fn send(&self, request: &OutboundHttpRequest) -> Result<OutboundHttpResponse, String> {
        if !self.allow_network {
            return Err("outbound network is disabled for this runtime".to_string());
        }

        let endpoint = self.resolve_endpoint(&request.integration)?;
        if request.url != endpoint.as_str() {
            return Err(format!(
                "integration `{}` must target the approved endpoint `{}`",
                request.integration,
                endpoint
            ));
        }

        let method = normalized_method(&request.method)?;
        let byte_limit = self.response_byte_limit(0)?;
        let endpoint_string = endpoint.to_string();
        let headers = request.headers.clone();
        let body = request.body.clone();
        let client = self.client.clone();
        let request_timeout = self.request_timeout;
        offload_outbound_http_to_blocking_pool(move || {
            perform_sdk_request(
                client,
                method,
                endpoint,
                endpoint_string,
                headers,
                body,
                request_timeout,
                byte_limit,
            )
        })
    }

    fn resolve_endpoint(&self, integration: &str) -> Result<Url, String> {
        // The guest only names an integration here; the backend resolves it to an approved
        // endpoint declared in config. Raw absolute URLs are rejected to keep guest-controlled
        // SSRF out of the host.
        if let Some(endpoint) = self.targets.get(integration) {
            return Ok(endpoint.clone());
        }

        Err(format!(
            "integration `{integration}` is not mapped to an outbound HTTP endpoint"
        ))
    }

    fn response_byte_limit(&self, response_bytes_hint: u64) -> Result<u64, String> {
        // The hint is advisory input from the guest. We treat it as an upper bound, then clamp
        // it to a backend-wide ceiling so a single request cannot exhaust memory.
        let hinted = if response_bytes_hint == 0 {
            self.max_response_bytes
        } else {
            response_bytes_hint
        };
        let limit = hinted
            .min(self.max_response_bytes)
            .min(MAX_RESPONSE_BYTES_FROM_HINT);
        if limit == 0 {
            return Err("outbound HTTP responses must allow at least one byte".to_string());
        }
        Ok(limit)
    }
}

fn perform_request(
    client: Agent,
    endpoint: Url,
    endpoint_string: String,
    integration: String,
    request_timeout: Duration,
    byte_limit: u64,
) -> Result<NetworkExecution, String> {
    let response = client
        .get(endpoint.as_str())
        .timeout(request_timeout)
        .call()
        .map_err(|error| format!("failed to call `{endpoint}`: {error}"))?;
    let status = response.status();
    let mut reader = response.into_reader().take(byte_limit.saturating_add(1));
    let mut response_bytes = Vec::new();
    reader
        .read_to_end(&mut response_bytes)
        .map_err(|error| format!("failed to read `{endpoint}` response body: {error}"))?;

    if response_bytes.len() as u64 > byte_limit {
        return Err(format!(
            "response from `{endpoint}` exceeded the configured limit of {byte_limit} bytes"
        ));
    }

    Ok(NetworkExecution {
        integration,
        endpoint: endpoint_string,
        status,
        response_bytes: response_bytes.len() as u64,
    })
}

fn normalized_method(method: &str) -> Result<String, String> {
    let normalized = method.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" => Ok(normalized),
        _ => Err(format!("unsupported outbound HTTP method `{method}`")),
    }
}

fn perform_sdk_request(
    client: Agent,
    method: String,
    endpoint: Url,
    endpoint_string: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
    request_timeout: Duration,
    byte_limit: u64,
) -> Result<OutboundHttpResponse, String> {
    let mut builder = client
        .request(&method, &endpoint_string)
        .timeout(request_timeout);
    for (name, value) in &headers {
        if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
            return Err(format!(
                "outbound HTTP request cannot override reserved header `{name}`"
            ));
        }
        builder = builder.set(name, value);
    }

    let response = if body.is_empty() {
        builder
            .call()
            .map_err(|error| format!("failed to call `{endpoint}`: {error}"))?
    } else {
        builder
            .send_bytes(&body)
            .map_err(|error| format!("failed to call `{endpoint}`: {error}"))?
    };
    let status = response.status();
    let mut response_headers = BTreeMap::new();
    for name in response.headers_names() {
        let values = response.all(&name);
        if !values.is_empty() {
            response_headers.insert(name, values.join(", "));
        }
    }

    let mut reader = response.into_reader().take(byte_limit.saturating_add(1));
    let mut response_bytes = Vec::new();
    reader
        .read_to_end(&mut response_bytes)
        .map_err(|error| format!("failed to read `{endpoint}` response body: {error}"))?;

    if response_bytes.len() as u64 > byte_limit {
        return Err(format!(
            "response from `{endpoint}` exceeded the configured limit of {byte_limit} bytes"
        ));
    }

    Ok(OutboundHttpResponse {
        status,
        headers: response_headers,
        body: response_bytes,
    })
}

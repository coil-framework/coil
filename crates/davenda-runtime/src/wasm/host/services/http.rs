use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use davenda_wasm::NetworkExecution;
use ureq::{Agent, AgentBuilder};
use url::Url;

use super::super::*;
use super::keys;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES_FROM_HINT: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(super) struct RuntimeOutboundHttpBackend {
    allow_network: bool,
    targets: Arc<BTreeMap<String, String>>,
    client: Agent,
    request_timeout: Duration,
    max_response_bytes: u64,
}

impl RuntimeOutboundHttpBackend {
    pub(super) fn new(allow_network: bool) -> Self {
        Self::with_targets(allow_network, BTreeMap::new())
    }

    pub(super) fn with_targets(allow_network: bool, targets: BTreeMap<String, String>) -> Self {
        Self::with_settings(
            allow_network,
            targets,
            DEFAULT_REQUEST_TIMEOUT,
            DEFAULT_MAX_RESPONSE_BYTES,
        )
    }

    pub(super) fn with_settings(
        allow_network: bool,
        targets: BTreeMap<String, String>,
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

    pub(super) fn execute(
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
        let response = self
            .client
            .get(&endpoint)
            .timeout(self.request_timeout)
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
            integration: integration.to_string(),
            endpoint,
            status,
            response_bytes: response_bytes.len() as u64,
        })
    }

    fn resolve_endpoint(&self, integration: &str) -> Result<String, String> {
        // The guest only names an integration here; the backend resolves it to an approved
        // endpoint. Raw absolute URLs are rejected to keep guest-controlled SSRF out of the host.
        if let Some(endpoint) = self.targets.get(integration) {
            validate_approved_endpoint(integration, endpoint)?;
            return Ok(endpoint.clone());
        }

        let env_key = integration_env_key(integration);
        let endpoint = std::env::var(&env_key).map_err(|_| {
            format!("integration `{integration}` is not mapped to an outbound HTTP endpoint")
        })?;
        validate_approved_endpoint(integration, &endpoint)?;
        Ok(endpoint)
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

fn validate_approved_endpoint(integration: &str, endpoint: &str) -> Result<(), String> {
    let parsed = Url::parse(endpoint).map_err(|error| {
        format!(
            "integration `{integration}` is mapped to an invalid outbound HTTP endpoint `{endpoint}`: {error}"
        )
    })?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "integration `{integration}` is mapped to unsupported outbound HTTP scheme `{scheme}`"
            ));
        }
    }

    if parsed.host_str().is_none() {
        return Err(format!(
            "integration `{integration}` is mapped to an outbound HTTP endpoint without a host"
        ));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(format!(
            "integration `{integration}` is mapped to an outbound HTTP endpoint with embedded credentials"
        ));
    }

    Ok(())
}

fn integration_env_key(integration: &str) -> String {
    format!("DAVENDA_WASM_HTTP_{}", keys::env_key_component(integration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn execution_context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("showcase-events")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-network").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/network", davenda_wasm::HttpMethod::Get).unwrap(),
            ),
        )
    }

    fn spawn_http_server(
        body: &'static str,
        delay: Option<Duration>,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            if let Some(delay) = delay {
                std::thread::sleep(delay);
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        });

        (endpoint, handle)
    }

    #[test]
    fn runtime_outbound_http_backend_requires_explicit_mappings() {
        let backend = RuntimeOutboundHttpBackend::with_settings(
            true,
            BTreeMap::new(),
            Duration::from_millis(100),
            1024,
        );

        let error = backend
            .execute("http://127.0.0.1:8080", 64, &execution_context())
            .unwrap_err();

        assert!(error.contains("not mapped"), "unexpected error: {error}");
    }

    #[test]
    fn runtime_outbound_http_backend_uses_explicit_endpoint_mappings() {
        let (endpoint, server) = spawn_http_server("mapped-response", None);
        let mut targets = BTreeMap::new();
        targets.insert("crm".to_string(), endpoint.clone());

        let backend =
            RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_secs(1), 1024);
        let execution = backend
            .execute("crm", 64, &execution_context())
            .expect("mapped endpoint should succeed");

        assert_eq!(execution.integration, "crm");
        assert_eq!(execution.endpoint, endpoint);
        assert_eq!(execution.status, 200);
        assert_eq!(execution.response_bytes, "mapped-response".len() as u64);

        server.join().unwrap();
    }

    #[test]
    fn runtime_outbound_http_backend_enforces_response_size_limits() {
        let (endpoint, server) = spawn_http_server("too-many-bytes", None);
        let mut targets = BTreeMap::new();
        targets.insert("search".to_string(), endpoint);

        let backend =
            RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_secs(1), 1024);
        let error = backend
            .execute("search", 4, &execution_context())
            .unwrap_err();

        assert!(
            error.contains("exceeded the configured limit"),
            "unexpected error: {error}"
        );

        server.join().unwrap();
    }

    #[test]
    fn runtime_outbound_http_backend_times_out_slow_endpoints() {
        let (endpoint, server) =
            spawn_http_server("slow-response", Some(Duration::from_millis(200)));
        let mut targets = BTreeMap::new();
        targets.insert("billing".to_string(), endpoint);

        let backend = RuntimeOutboundHttpBackend::with_settings(
            true,
            targets,
            Duration::from_millis(25),
            1024,
        );
        let error = backend
            .execute("billing", 64, &execution_context())
            .unwrap_err();

        assert!(error.contains("timed out"), "unexpected error: {error}");

        server.join().unwrap();
    }
}

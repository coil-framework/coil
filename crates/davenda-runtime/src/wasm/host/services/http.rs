use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

use davenda_wasm::NetworkExecution;
use ureq::{Agent, AgentBuilder};

use super::super::*;
use super::keys;

#[derive(Debug, Clone)]
pub(super) struct RuntimeOutboundHttpBackend {
    allow_network: bool,
    targets: Arc<BTreeMap<String, String>>,
    client: Agent,
}

impl RuntimeOutboundHttpBackend {
    pub(super) fn new(allow_network: bool) -> Self {
        Self::with_targets(allow_network, BTreeMap::new())
    }

    pub(super) fn with_targets(allow_network: bool, targets: BTreeMap<String, String>) -> Self {
        Self {
            allow_network,
            targets: Arc::new(targets),
            client: AgentBuilder::new().redirects(4).build(),
        }
    }

    pub(super) fn execute(
        &self,
        integration: &str,
        _response_bytes_hint: u64,
        _context: &InvocationContext,
    ) -> Result<NetworkExecution, String> {
        if !self.allow_network {
            return Err("outbound network is disabled for this runtime".to_string());
        }

        let endpoint = self.resolve_endpoint(integration)?;
        let response = self
            .client
            .get(&endpoint)
            .call()
            .map_err(|error| format!("failed to call `{endpoint}`: {error}"))?;
        let status = response.status();
        let mut reader = response.into_reader();
        let mut response_bytes = Vec::new();
        reader
            .read_to_end(&mut response_bytes)
            .map_err(|error| format!("failed to read `{endpoint}` response body: {error}"))?;

        Ok(NetworkExecution {
            integration: integration.to_string(),
            endpoint,
            status,
            response_bytes: response_bytes.len() as u64,
        })
    }

    fn resolve_endpoint(&self, integration: &str) -> Result<String, String> {
        if integration.starts_with("http://") || integration.starts_with("https://") {
            return Ok(integration.to_string());
        }

        if let Some(endpoint) = self.targets.get(integration) {
            return Ok(endpoint.clone());
        }

        let env_key = integration_env_key(integration);
        std::env::var(&env_key).map_err(|_| {
            format!("integration `{integration}` is not mapped to an outbound HTTP endpoint")
        })
    }
}

fn integration_env_key(integration: &str) -> String {
    format!("DAVENDA_WASM_HTTP_{}", keys::env_key_component(integration))
}

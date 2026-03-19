use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use davenda_jobs::{JobId, JobInstant, JobName, JobQueueName, JobSpec};
use davenda_wasm::{
    JobExecution, MetadataExecution, MetadataGrant, NetworkExecution, SecretExecution,
};
use reqwest::blocking::Client;

use super::super::*;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeWasmHostServices {
    http: RuntimeOutboundHttpBackend,
    secrets: RuntimeSecretBackend,
    jobs: RuntimeJobBackend,
    metadata: RuntimeMetadataJournal,
}

impl RuntimeWasmHostServices {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        Self {
            http: RuntimeOutboundHttpBackend::new(plan.wasm.allow_network),
            secrets: RuntimeSecretBackend::default(),
            jobs: RuntimeJobBackend::new(plan),
            metadata: RuntimeMetadataJournal::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_backends(
        plan: RuntimePlan,
        http_targets: BTreeMap<String, String>,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        Self {
            http: RuntimeOutboundHttpBackend::with_targets(plan.wasm.allow_network, http_targets),
            secrets: RuntimeSecretBackend::with_values(secrets),
            jobs: RuntimeJobBackend::new(plan),
            metadata: RuntimeMetadataJournal::default(),
        }
    }

    pub(crate) fn execute_http(
        &self,
        integration: &str,
        response_bytes_hint: u64,
        context: &InvocationContext,
    ) -> Result<NetworkExecution, String> {
        self.http.execute(integration, response_bytes_hint, context)
    }

    pub(crate) fn read_secret(
        &self,
        secret: &str,
        context: &InvocationContext,
    ) -> Result<SecretExecution, String> {
        self.secrets.read(secret, context)
    }

    pub(crate) fn enqueue_job(
        &self,
        queue: &str,
        context: &InvocationContext,
    ) -> Result<JobExecution, String> {
        self.jobs.enqueue(queue, context)
    }

    pub(crate) fn record_metadata_write(
        &self,
        kind: MetadataGrant,
        context: &InvocationContext,
    ) -> Result<MetadataExecution, String> {
        self.metadata.record(kind, context)
    }

    #[cfg(test)]
    pub(crate) fn metadata_records(&self) -> Vec<MetadataWriteRecord> {
        self.metadata.records()
    }
}

#[derive(Debug, Clone)]
struct RuntimeOutboundHttpBackend {
    allow_network: bool,
    targets: Arc<BTreeMap<String, String>>,
    client: Client,
}

impl RuntimeOutboundHttpBackend {
    fn new(allow_network: bool) -> Self {
        Self::with_targets(allow_network, BTreeMap::new())
    }

    fn with_targets(allow_network: bool, targets: BTreeMap<String, String>) -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(4))
            .build()
            .expect("static reqwest client configuration must be valid");

        Self {
            allow_network,
            targets: Arc::new(targets),
            client,
        }
    }

    fn execute(
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
            .send()
            .map_err(|error| format!("failed to call `{endpoint}`: {error}"))?;
        let status = response.status().as_u16();
        let response_bytes = response
            .bytes()
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

#[derive(Debug, Clone, Default)]
struct RuntimeSecretBackend {
    values: Arc<BTreeMap<String, String>>,
}

impl RuntimeSecretBackend {
    #[cfg(test)]
    fn with_values(values: BTreeMap<String, String>) -> Self {
        Self {
            values: Arc::new(values),
        }
    }

    fn read(&self, secret: &str, _context: &InvocationContext) -> Result<SecretExecution, String> {
        if let Some(value) = self.values.get(secret) {
            return Ok(SecretExecution {
                secret: secret.to_string(),
                source: format!("in-memory:{secret}"),
                value_bytes: value.len(),
            });
        }

        let env_key = secret_env_key(secret);
        let value = std::env::var(&env_key)
            .map_err(|_| format!("secret `{secret}` was not provided to the runtime"))?;

        Ok(SecretExecution {
            secret: secret.to_string(),
            source: format!("env:{env_key}"),
            value_bytes: value.len(),
        })
    }
}

#[derive(Debug, Clone)]
struct RuntimeJobBackend {
    plan: RuntimePlan,
    sequence: Arc<AtomicU64>,
}

impl RuntimeJobBackend {
    fn new(plan: RuntimePlan) -> Self {
        Self {
            plan,
            sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    fn enqueue(&self, queue: &str, context: &InvocationContext) -> Result<JobExecution, String> {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let queue = JobQueueName::new(queue.to_string()).map_err(|error| error.to_string())?;
        let job_id = JobId::new(format!(
            "wasm:{}:{}:{}",
            context.trace.trace_id, queue, seq
        ))
        .map_err(|error| error.to_string())?;
        let job_name = JobName::new(format!("wasm.enqueue.{queue}")).map_err(|error| error.to_string())?;
        let payload_description = format!(
            "wasm host enqueue for queue `{queue}` from trace `{}`",
            context.trace.trace_id
        );
        let spec = JobSpec::new(job_id.clone(), job_name, queue.clone(), payload_description)
            .map_err(|error| error.to_string())?;

        let now = current_job_instant();
        let mut host = self
            .plan
            .jobs_host("wasm-host")
            .map_err(|error| error.to_string())?;
        let _ = host
            .enqueue_spec(spec, now)
            .map_err(|error| error.to_string())?;

        Ok(JobExecution {
            queue: queue.to_string(),
            job_id: job_id.to_string(),
            enqueued_at_unix_seconds: now.as_unix_seconds(),
        })
    }
}

#[derive(Debug, Clone, Default)]
struct RuntimeMetadataJournal {
    entries: Arc<Mutex<Vec<MetadataWriteRecord>>>,
}

impl RuntimeMetadataJournal {
    fn record(
        &self,
        kind: MetadataGrant,
        context: &InvocationContext,
    ) -> Result<MetadataExecution, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "metadata journal is poisoned".to_string())?;
        entries.push(MetadataWriteRecord {
            kind,
            trace_id: context.trace.trace_id.clone(),
            app_id: context.customer_app.app_id.clone(),
        });

        Ok(MetadataExecution {
            kind,
            recorded: true,
            journal_entries: entries.len(),
        })
    }

    #[cfg(test)]
    fn records(&self) -> Vec<MetadataWriteRecord> {
        self.entries
            .lock()
            .expect("metadata journal poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataWriteRecord {
    pub kind: MetadataGrant,
    pub trace_id: String,
    pub app_id: String,
}

fn integration_env_key(integration: &str) -> String {
    format!("DAVENDA_WASM_HTTP_{}", env_key_component(integration))
}

fn secret_env_key(secret: &str) -> String {
    format!("DAVENDA_WASM_SECRET_{}", env_key_component(secret))
}

fn env_key_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut last_was_underscore = false;

    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_uppercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if last_was_underscore {
                continue;
            }
            last_was_underscore = true;
        } else {
            last_was_underscore = false;
        }

        sanitized.push(mapped);
    }

    while sanitized.starts_with('_') {
        sanitized.remove(0);
    }
    while sanitized.ends_with('_') {
        sanitized.pop();
    }

    if sanitized.is_empty() {
        "DEFAULT".to_string()
    } else {
        sanitized
    }
}

fn current_job_instant() -> JobInstant {
    JobInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

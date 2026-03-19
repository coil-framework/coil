use std::collections::BTreeMap;

use davenda_wasm::{
    JobExecution, MetadataExecution, MetadataGrant, NetworkExecution, SecretExecution,
};

use super::super::*;

mod http;
mod jobs;
mod keys;
mod metadata;
mod secrets;

pub(crate) use metadata::MetadataWriteRecord;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeWasmHostServices {
    http: http::RuntimeOutboundHttpBackend,
    secrets: secrets::RuntimeSecretBackend,
    jobs: jobs::RuntimeJobBackend,
    metadata: metadata::RuntimeMetadataBackend,
}

impl RuntimeWasmHostServices {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        Self {
            http: http::RuntimeOutboundHttpBackend::new(plan.wasm.allow_network),
            secrets: secrets::RuntimeSecretBackend::default(),
            jobs: jobs::RuntimeJobBackend::new(plan),
            metadata: metadata::RuntimeMetadataBackend::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_backends(
        plan: RuntimePlan,
        http_targets: BTreeMap<String, String>,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                http_targets,
            ),
            secrets: secrets::RuntimeSecretBackend::with_values(secrets),
            jobs: jobs::RuntimeJobBackend::new(plan),
            metadata: metadata::RuntimeMetadataBackend::default(),
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

use davenda_wasm::{
    JobExecution, MetadataExecution, MetadataGrant, NetworkExecution, SecretExecution,
};

use super::super::*;

use std::collections::BTreeMap;

mod http;
mod jobs;
mod metadata;
mod secrets;

pub(crate) use metadata::{MetadataAuditRecord, MetadataAuditSnapshot};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeWasmHostServices {
    http: http::RuntimeOutboundHttpBackend,
    secrets: secrets::RuntimeSecretBackend,
    jobs: jobs::RuntimeJobBackend,
    metadata: metadata::RuntimeMetadataBackend,
}

impl RuntimeWasmHostServices {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        #[cfg(test)]
        let metadata =
            metadata::RuntimeMetadataBackend::open_for_test(plan.shared_backend_namespace());
        #[cfg(not(test))]
        let metadata = metadata::RuntimeMetadataBackend::open(
            plan.config.storage.local_root.clone(),
            plan.shared_backend_namespace(),
        );
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                plan.approved_outbound_http_endpoints().clone(),
            ),
            secrets: secrets::RuntimeSecretBackend::deny_all(plan.config.app.name.clone()),
            jobs,
            metadata,
        }
    }

    pub(crate) fn with_runtime_secrets(
        plan: RuntimePlan,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        #[cfg(test)]
        let metadata =
            metadata::RuntimeMetadataBackend::open_for_test(plan.shared_backend_namespace());
        #[cfg(not(test))]
        let metadata = metadata::RuntimeMetadataBackend::open(
            plan.config.storage.local_root.clone(),
            plan.shared_backend_namespace(),
        );
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                plan.approved_outbound_http_endpoints().clone(),
            ),
            secrets: secrets::RuntimeSecretBackend::runtime_scoped(
                plan.config.app.name.clone(),
                secrets,
            ),
            jobs,
            metadata,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shared_state_root(
        root: impl Into<std::path::PathBuf>,
        plan: RuntimePlan,
        http_targets: BTreeMap<String, url::Url>,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        let metadata =
            metadata::RuntimeMetadataBackend::with_root(root, plan.shared_backend_namespace());
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                http_targets,
            ),
            secrets: secrets::RuntimeSecretBackend::with_values(secrets),
            jobs,
            metadata,
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

    pub(crate) fn metadata_records(
        &self,
        limit: usize,
    ) -> Result<Vec<MetadataAuditRecord>, String> {
        self.metadata.recent_records(limit)
    }
}

use davenda_wasm::{
    JobExecution, MetadataExecution, MetadataGrant, NetworkExecution, SecretExecution,
};

use super::super::*;

use std::collections::BTreeMap;

mod http;
mod jobs;
mod metadata;
mod secrets;
mod webhooks;

pub(crate) use metadata::MetadataAuditSnapshot;
pub use webhooks::{
    WebhookObservationBackendKind, WebhookObservationEvent, WebhookObservationSnapshot,
    WebhookObservationStatus, WebhookObservationStatusCounts,
};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeWasmHostServices {
    http: http::RuntimeOutboundHttpBackend,
    secrets: secrets::RuntimeSecretBackend,
    jobs: jobs::RuntimeJobBackend,
    metadata: metadata::RuntimeMetadataBackend,
    webhooks: webhooks::RuntimeWebhookObservationBackend,
    storage: StorageHost,
}

impl RuntimeWasmHostServices {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        let metadata = metadata::RuntimeMetadataBackend::open(&plan);
        let webhooks = webhooks::RuntimeWebhookObservationBackend::open(&plan);
        let storage = plan.storage_host();
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                plan.approved_outbound_http_endpoints().clone(),
            ),
            secrets: secrets::RuntimeSecretBackend::deny_all(plan.config.app.name.clone()),
            jobs,
            metadata,
            webhooks,
            storage,
        }
    }

    pub(crate) fn with_runtime_secrets(
        plan: RuntimePlan,
        storage: StorageHost,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        let metadata = metadata::RuntimeMetadataBackend::open(&plan);
        let webhooks = webhooks::RuntimeWebhookObservationBackend::open(&plan);
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
            webhooks,
            storage,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_shared_state_root(
        root: impl Into<std::path::PathBuf>,
        plan: RuntimePlan,
        http_targets: BTreeMap<String, url::Url>,
        secrets: BTreeMap<String, String>,
    ) -> Self {
        let root = root.into();
        let jobs = jobs::RuntimeJobBackend::new(plan.clone());
        let metadata = metadata::RuntimeMetadataBackend::with_local_root(
            root.clone(),
            plan.shared_backend_namespace(),
        );
        let webhooks = webhooks::RuntimeWebhookObservationBackend::with_local_root(
            root,
            plan.shared_backend_namespace(),
        );
        let storage = plan.storage_host();
        Self {
            http: http::RuntimeOutboundHttpBackend::with_targets(
                plan.wasm.allow_network,
                http_targets,
            ),
            secrets: secrets::RuntimeSecretBackend::with_values(secrets),
            jobs,
            metadata,
            webhooks,
            storage,
        }
    }

    pub(crate) fn submit_outbound_http_to_blocking_pool(
        &self,
        integration: &str,
        response_bytes_hint: u64,
        context: &InvocationContext,
    ) -> Result<NetworkExecution, String> {
        self.http
            .submit_outbound_http_to_blocking_pool(integration, response_bytes_hint, context)
    }

    pub(crate) fn send_outbound_http(
        &self,
        request: &davenda_customer_sdk::OutboundHttpRequest,
    ) -> Result<davenda_customer_sdk::OutboundHttpResponse, String> {
        self.http.send(request)
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

    pub(crate) fn metadata_snapshot(
        &self,
        limit: usize,
    ) -> Result<metadata::MetadataAuditSnapshot, String> {
        self.metadata.snapshot(limit)
    }

    pub(crate) fn metadata_backend_kind(&self) -> metadata::MetadataAuditBackendKind {
        self.metadata.backend_kind()
    }

    pub(crate) fn metadata_location(&self) -> String {
        self.metadata.location_label()
    }

    pub(crate) fn record_operator_audit(
        &self,
        kind: impl Into<String>,
        app_id: &str,
        request_id: Option<&str>,
        principal_id: Option<&str>,
    ) -> Result<(), String> {
        self.metadata
            .record_operator_action(kind, app_id, request_id, principal_id)
    }

    pub(crate) fn record_webhook_observation(
        &self,
        source: &str,
        event: &str,
        status: WebhookObservationStatus,
        context: &InvocationContext,
        detail: Option<String>,
    ) -> Result<(), String> {
        self.webhooks.record(source, event, status, context, detail)
    }

    pub(crate) fn webhook_observation_snapshot(
        &self,
        limit: usize,
    ) -> Result<WebhookObservationSnapshot, String> {
        self.webhooks.snapshot(limit)
    }

    pub(crate) fn storage_host(&self) -> &StorageHost {
        &self.storage
    }
}

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::executor::RuntimeHostServiceExecutor;
use super::support::{http_method_to_wasm, invocation_surface_path};
use super::*;
use sha2::{Digest, Sha256};
use thiserror::Error;

mod cache;
mod context;
mod prepare;
mod principal;
mod services;

pub use principal::ExtensionPrincipal;
pub(crate) use services::{MetadataAuditSnapshot, RuntimeWasmHostServices};
pub use services::{
    WebhookObservationBackendKind, WebhookObservationEvent, WebhookObservationSnapshot,
    WebhookObservationStatus, WebhookObservationStatusCounts,
};

#[derive(Debug, Error)]
pub enum LiveWasmExecutionError {
    #[error(transparent)]
    Model(#[from] WasmModelError),
    #[error(
        "failed to read installed extension artifact for `{extension_id}` at `{path}`: {reason}"
    )]
    ArtifactRead {
        extension_id: String,
        path: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredExtensionSlot {
    pub module: String,
    pub kind: ExtensionPointKind,
    pub surface: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtensionSummary {
    pub extension_id: String,
    pub display_name: String,
    pub customer_app_id: String,
    pub handler_count: usize,
}

#[derive(Debug, Clone)]
pub struct WasmHost {
    pub customer_app: String,
    pub runtime: WasmRuntimeServices,
    registry: ExtensionRegistry,
    engine: WasmEngine,
    tenant_id: i64,
    default_locale: String,
    registered_jobs: Vec<RuntimeJobDefinition>,
    host_services: RuntimeWasmHostServices,
    host_service_executor: Arc<dyn HostServiceExecutor>,
    compiled_modules: Arc<cache::CompiledModuleCache<String, CompiledWasmModule>>,
}

impl WasmHost {
    pub(crate) fn new(
        plan: RuntimePlan,
        customer_app: String,
        runtime: WasmRuntimeServices,
        registry: ExtensionRegistry,
        default_locale: String,
        registered_jobs: Vec<RuntimeJobDefinition>,
    ) -> Self {
        let host_services = RuntimeWasmHostServices::new(plan.clone());
        let host_service_executor = Arc::new(RuntimeHostServiceExecutor::with_services(
            plan.clone(),
            host_services.clone(),
        ));
        Self {
            customer_app,
            runtime,
            registry,
            engine: WasmEngine::new(),
            tenant_id: plan.tenant_id(),
            default_locale,
            registered_jobs,
            host_services,
            host_service_executor,
            compiled_modules: Arc::new(cache::CompiledModuleCache::default()),
        }
    }

    pub(crate) fn with_host_services(
        plan: RuntimePlan,
        customer_app: String,
        runtime: WasmRuntimeServices,
        registry: ExtensionRegistry,
        default_locale: String,
        registered_jobs: Vec<RuntimeJobDefinition>,
        services: RuntimeWasmHostServices,
    ) -> Self {
        let host_service_executor = Arc::new(RuntimeHostServiceExecutor::with_services(
            plan.clone(),
            services.clone(),
        ));
        Self {
            customer_app,
            runtime,
            registry,
            engine: WasmEngine::new(),
            tenant_id: plan.tenant_id(),
            default_locale,
            registered_jobs,
            host_services: services,
            host_service_executor,
            compiled_modules: Arc::new(cache::CompiledModuleCache::default()),
        }
    }

    pub(crate) fn metadata_audit_snapshot(
        &self,
        limit: usize,
    ) -> Result<MetadataAuditSnapshot, String> {
        self.host_services.metadata_snapshot(limit)
    }

    pub(crate) fn metadata_audit_backend_kind(&self) -> &'static str {
        self.host_services.metadata_backend_kind().as_str()
    }

    pub(crate) fn metadata_audit_location(&self) -> String {
        self.host_services.metadata_location()
    }

    pub(crate) fn upsert_customer_managed_asset(
        &self,
        logical_path: &str,
        record_json: &str,
        updated_at_unix_seconds: i64,
    ) -> Result<(), String> {
        self.host_services.upsert_customer_managed_asset(
            logical_path,
            record_json,
            updated_at_unix_seconds,
        )
    }

    pub(crate) fn customer_managed_asset(
        &self,
        logical_path: &str,
    ) -> Result<Option<String>, String> {
        self.host_services.customer_managed_asset(logical_path)
    }

    pub(crate) fn record_operator_audit(
        &self,
        kind: impl Into<String>,
        app_id: &str,
        request_id: Option<&str>,
        principal_id: Option<&str>,
    ) -> Result<(), String> {
        self.host_services
            .record_operator_audit(kind, app_id, request_id, principal_id)
    }

    pub(crate) fn send_outbound_http(
        &self,
        request: &davenda_customer_sdk::OutboundHttpRequest,
    ) -> Result<davenda_customer_sdk::OutboundHttpResponse, String> {
        self.host_services.send_outbound_http(request)
    }

    pub fn webhook_observation_snapshot(
        &self,
        limit: usize,
    ) -> Result<WebhookObservationSnapshot, String> {
        self.host_services.webhook_observation_snapshot(limit)
    }

    pub fn compile_module(&self, bytes: &[u8]) -> Result<CompiledWasmModule, WasmModelError> {
        self.engine.compile_module(bytes)
    }

    pub fn execute_session(
        &self,
        module: &CompiledWasmModule,
        session: WasmExecutionSession,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        let export = self
            .registry
            .handler_export(&session.plan().extension_id, &session.plan().handler_id)
            .ok_or_else(|| WasmModelError::HandlerNotFound {
                handler_id: session.plan().handler_id.to_string(),
            })?;
        self.engine.execute_session(module, session, export)
    }

    pub fn execute_request_surface(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<ExecutionReceipt>, LiveWasmExecutionError> {
        match execution.route_area {
            RouteArea::Api => self
                .begin_api_invocation(execution)?
                .map(|session| self.execute_installed_session(session))
                .transpose(),
            _ => self
                .begin_page_invocation(execution)?
                .map(|session| self.execute_installed_session(session))
                .transpose(),
        }
    }

    pub fn execute_leased_job(
        &self,
        lease: &JobLease,
    ) -> Result<Option<ExecutionReceipt>, LiveWasmExecutionError> {
        self.begin_leased_job_invocation(lease)?
            .map(|session| self.execute_installed_session(session))
            .transpose()
    }

    pub fn execute_render_hook_slot(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<ExecutionReceipt>, LiveWasmExecutionError> {
        let sessions = self.begin_render_hook_invocations(slot, execution)?;
        let mut receipts = Vec::with_capacity(sessions.len());
        for session in sessions {
            receipts.push(self.execute_installed_session(session)?);
        }
        Ok(receipts)
    }

    pub fn execute_admin_widget_slot(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<ExecutionReceipt>, LiveWasmExecutionError> {
        let sessions = self.begin_admin_widget_invocations(slot, execution)?;
        let mut receipts = Vec::with_capacity(sessions.len());
        for session in sessions {
            receipts.push(self.execute_installed_session(session)?);
        }
        Ok(receipts)
    }

    fn execute_installed_session(
        &self,
        session: WasmExecutionSession,
    ) -> Result<ExecutionReceipt, LiveWasmExecutionError> {
        let module = self.load_installed_module(&session.plan().extension_id)?;
        self.execute_session(module.as_ref(), session)
            .map_err(Into::into)
    }

    fn load_installed_module(
        &self,
        extension_id: &davenda_wasm::ExtensionId,
    ) -> Result<Arc<CompiledWasmModule>, LiveWasmExecutionError> {
        let bytes = if let Some(installed) = self.registry.extension(extension_id) {
            if let Some(artifact) = installed.artifact() {
                let cache_key = artifact.compiled_module_cache_key().to_string();
                let bytes = artifact.load_bytes(&self.runtime.extension_directory, extension_id)?;
                return self.compiled_modules.get_or_insert_with(cache_key, || {
                    self.compile_module(&bytes).map_err(Into::into)
                });
            } else {
                let path = self.installed_module_path(extension_id);
                fs::read(&path).map_err(|error| LiveWasmExecutionError::ArtifactRead {
                    extension_id: extension_id.to_string(),
                    path: path.display().to_string(),
                    reason: error.to_string(),
                })?
            }
        } else {
            let path = self.installed_module_path(extension_id);
            fs::read(&path).map_err(|error| LiveWasmExecutionError::ArtifactRead {
                extension_id: extension_id.to_string(),
                path: path.display().to_string(),
                reason: error.to_string(),
            })?
        };

        let cache_key = format!("{:x}", Sha256::digest(&bytes));
        self.compiled_modules.get_or_insert_with(cache_key, || {
            self.compile_module(&bytes).map_err(Into::into)
        })
    }

    fn installed_module_path(&self, extension_id: &davenda_wasm::ExtensionId) -> PathBuf {
        PathBuf::from(&self.runtime.extension_directory).join(format!("{extension_id}.wasm"))
    }
}

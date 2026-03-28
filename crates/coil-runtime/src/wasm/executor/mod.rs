mod auth;
mod data;
mod render;
mod services;

use super::auth_backend::RuntimeAuthBackend;
use super::host::RuntimeWasmHostServices;
use super::support::{
    runtime_auth_backend_error, runtime_data_backend_error, runtime_executor_error,
    runtime_host_service_error, storage_class_from_grant, trace_id,
};
use super::*;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(super) struct RuntimeHostServiceExecutor {
    plan: RuntimePlan,
    auth_backend: OnceLock<Result<RuntimeAuthBackend, String>>,
    data_backend: OnceLock<Result<RuntimeDataBackend, String>>,
    services: RuntimeWasmHostServices,
}

impl RuntimeHostServiceExecutor {
    pub(super) fn with_services(plan: RuntimePlan, services: RuntimeWasmHostServices) -> Self {
        Self {
            services,
            plan,
            auth_backend: OnceLock::new(),
            data_backend: OnceLock::new(),
        }
    }

    fn host_service_execution(
        &self,
        call: &HostServiceCall,
        result: HostServiceResult,
    ) -> HostServiceExecution {
        HostServiceExecution {
            call: call.clone(),
            result,
        }
    }

    fn record_observability(
        &self,
        span: &str,
        trace_id: &str,
        outcome: &str,
        duration_ms: u64,
    ) {
        let telemetry = &self.plan.observability.telemetry;
        let _ = telemetry.record_histogram("coil.extension.runtime_ms", duration_ms);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = telemetry.record_trace(
            coil_observability::TraceRecord::new(trace_id.to_string(), span, outcome, now)
                .with_field("customer_app", self.plan.config.app.name.clone())
                .with_field("duration_ms", duration_ms.to_string()),
        );
    }
}

impl HostServiceExecutor for RuntimeHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let span = match &call.request {
            HostServiceRequest::Auth(_) => "wasm.host.auth",
            HostServiceRequest::Data(_) => "wasm.host.data",
            HostServiceRequest::Storage(_) => "wasm.host.storage",
            HostServiceRequest::Render(_) => "wasm.host.render",
            HostServiceRequest::CacheIntent(_) => "wasm.host.cache_intent",
            HostServiceRequest::OutboundHttp { .. } => "wasm.host.outbound_http",
            HostServiceRequest::SecretRead { .. } => "wasm.host.secret_read",
            HostServiceRequest::EnqueueJob { .. } => "wasm.host.enqueue_job",
            HostServiceRequest::MetadataWrite { .. } => "wasm.host.metadata_write",
        };
        let started_at = Instant::now();
        let result = match &call.request {
            HostServiceRequest::Auth(request) => self.execute_auth(call, context, request),
            HostServiceRequest::Data(request) => self.execute_data(call, context, request),
            HostServiceRequest::Storage(request) => self.execute_storage(call, context, request),
            HostServiceRequest::Render(request) => self.execute_render(call, context, request),
            HostServiceRequest::CacheIntent(request) => {
                self.execute_cache_intent(call, context, request)
            }
            HostServiceRequest::OutboundHttp {
                integration,
                response_bytes,
            } => self.dispatch_outbound_http_to_blocking_pool(
                call,
                context,
                integration,
                *response_bytes,
            ),
            HostServiceRequest::SecretRead { secret } => self.execute_secret(call, context, secret),
            HostServiceRequest::EnqueueJob { queue } => self.execute_job(call, context, queue),
            HostServiceRequest::MetadataWrite { kind } => {
                self.execute_metadata(call, context, *kind)
            }
        };
        let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        self.record_observability(
            span,
            trace_id(context),
            if result.is_ok() { "ok" } else { "error" },
            elapsed_ms,
        );
        result
    }
}

#[cfg(test)]
mod tests;

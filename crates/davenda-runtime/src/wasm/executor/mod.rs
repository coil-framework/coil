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
}

impl HostServiceExecutor for RuntimeHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        match &call.request {
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
        }
    }
}

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};

use crate::grants::StorageClassGrant;
use crate::host_api::HostServiceRequest;
use crate::invocation::InvocationContext;

use super::auth::{AuthServiceExecution, auth_details_for_request};
use super::cache::{CacheIntentExecution, cache_key_for_request};
use super::data::{DataServiceExecution, data_summary_for_request};
use super::journal::HostServiceExecutor;
use super::render::{RenderServiceExecution, render_fragment_for_request, render_fragment_key};
use super::storage::{
    StorageServiceExecution, storage_description_for_request, storage_total_for_request,
};
use super::types::{
    HostServiceExecution, HostServiceResult, JobExecution, MetadataExecution, NetworkExecution,
    SecretExecution,
};

#[derive(Debug, Clone)]
pub struct SyntheticHostServiceExecutor {
    state: Arc<Mutex<SyntheticHostServiceState>>,
}

impl HostServiceExecutor for SyntheticHostServiceExecutor {
    fn execute(
        &self,
        call: &crate::host_api::HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, crate::error::WasmModelError> {
        let mut state = self
            .state
            .lock()
            .expect("synthetic host service state poisoned");
        Ok(state.execute(call.clone(), context))
    }
}

impl Default for SyntheticHostServiceExecutor {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(SyntheticHostServiceState::default())),
        }
    }
}

#[derive(Debug, Default)]
struct SyntheticHostServiceState {
    auth_checks: u32,
    data_sequences: u64,
    storage_bytes_by_class: std::collections::BTreeMap<StorageClassGrant, u64>,
    render_fragments: std::collections::BTreeMap<String, String>,
    cache_hints: Vec<String>,
    metadata_writes: usize,
    job_sequences: u64,
}

impl SyntheticHostServiceState {
    fn execute(
        &mut self,
        call: crate::host_api::HostServiceCall,
        context: &InvocationContext,
    ) -> HostServiceExecution {
        let result = match &call.request {
            HostServiceRequest::Auth(request) => {
                self.auth_checks = self.auth_checks.saturating_add(1);
                let principal_id = context.principal.id.clone();
                let details = auth_details_for_request(request, context, self.auth_checks);
                HostServiceResult::Auth(AuthServiceExecution {
                    request: request.clone(),
                    allowed: true,
                    checks_seen: self.auth_checks,
                    principal_id,
                    details,
                })
            }
            HostServiceRequest::Data(request) => {
                self.data_sequences = self.data_sequences.saturating_add(1);
                HostServiceResult::Data(DataServiceExecution {
                    request: request.clone(),
                    summary: data_summary_for_request(request, context, self.data_sequences),
                    sequence: self.data_sequences,
                })
            }
            HostServiceRequest::Storage(request) => {
                let total_bytes =
                    storage_total_for_request(request, &mut self.storage_bytes_by_class, context);
                HostServiceResult::Storage(StorageServiceExecution {
                    request: request.clone(),
                    description: storage_description_for_request(request, context, total_bytes),
                    total_bytes,
                })
            }
            HostServiceRequest::Render(request) => {
                let fragment = render_fragment_for_request(request, context);
                self.render_fragments
                    .insert(render_fragment_key(request, context), fragment.clone());
                HostServiceResult::Render(RenderServiceExecution {
                    request: request.clone(),
                    fragment,
                })
            }
            HostServiceRequest::CacheIntent(request) => {
                let cache_key = cache_key_for_request(request, context, self.cache_hints.len());
                self.cache_hints.push(cache_key.clone());
                HostServiceResult::CacheIntent(CacheIntentExecution {
                    request: request.clone(),
                    cache_key,
                    applied: true,
                })
            }
            HostServiceRequest::OutboundHttp {
                integration,
                response_bytes,
            } => HostServiceResult::Network(NetworkExecution {
                integration: integration.clone(),
                endpoint: format!("synthetic://{integration}"),
                status: 200,
                response_bytes: *response_bytes,
            }),
            HostServiceRequest::SecretRead { secret } => {
                HostServiceResult::Secret(SecretExecution {
                    secret: secret.clone(),
                    source: "synthetic".to_string(),
                    value_bytes: secret.len(),
                })
            }
            HostServiceRequest::EnqueueJob { queue } => {
                self.job_sequences = self.job_sequences.saturating_add(1);
                HostServiceResult::Job(JobExecution {
                    queue: queue.clone(),
                    job_id: format!("synthetic:{queue}:{}", self.job_sequences),
                    enqueued_at_unix_seconds: self.job_sequences,
                })
            }
            HostServiceRequest::MetadataWrite { kind } => {
                self.metadata_writes = self.metadata_writes.saturating_add(1);
                HostServiceResult::Metadata(MetadataExecution {
                    kind: *kind,
                    recorded: true,
                    journal_entries: self.metadata_writes,
                })
            }
        };

        HostServiceExecution { call, result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grants::HostCapabilityGrant;
    use crate::host_api::{AuthServiceRequest, HostServiceCall, HostServiceRequest};
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("synthetic-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-synthetic").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/synthetic", crate::ids::HttpMethod::Get).unwrap(),
            ),
        )
    }

    #[test]
    fn synthetic_executor_materializes_auth_results() {
        let executor = SyntheticHostServiceExecutor::default();
        let call = HostServiceCall::new(
            HostCapabilityGrant::AuthCheck,
            HostServiceRequest::Auth(AuthServiceRequest::Check),
        );
        let execution = executor.execute(&call, &context()).unwrap();
        match execution.result {
            HostServiceResult::Auth(result) => assert!(result.allowed),
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

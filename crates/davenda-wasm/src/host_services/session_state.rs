use std::sync::Arc;
use std::time::Duration;

use crate::error::WasmModelError;
use crate::grants::HostCapabilityGrant;
use crate::host_api::{
    AuthServiceRequest, CacheIntentServiceRequest, DataServiceRequest, HostServiceCall,
    HostServiceRequest, RenderServiceRequest, StorageServiceRequest,
};
use crate::host_services::{HostServiceExecution, HostServiceExecutor, HostServiceJournal};
use crate::invocation::{
    ExecutionReceipt, ExecutionUsage, HostCall, InvocationOutcome, InvocationPlan,
};
use crate::output::TypedExecutionOutput;

#[derive(Debug, Clone)]
pub struct HostServiceSessionState {
    plan: InvocationPlan,
    usage: ExecutionUsage,
    active_concurrency: u16,
    host_calls: Vec<HostCall>,
    host_service_journal: HostServiceJournal,
}

impl HostServiceSessionState {
    pub fn new(plan: InvocationPlan) -> Self {
        Self::with_executor(
            plan,
            Arc::new(crate::host_services::SyntheticHostServiceExecutor::default()),
        )
    }

    pub fn with_executor(plan: InvocationPlan, executor: Arc<dyn HostServiceExecutor>) -> Self {
        Self {
            plan,
            usage: ExecutionUsage::default(),
            active_concurrency: 0,
            host_calls: Vec::new(),
            host_service_journal: HostServiceJournal::with_executor(executor),
        }
    }

    pub fn plan(&self) -> &InvocationPlan {
        &self.plan
    }

    pub fn usage(&self) -> &ExecutionUsage {
        &self.usage
    }

    pub fn host_calls(&self) -> &[HostCall] {
        &self.host_calls
    }

    pub fn host_service_executions(&self) -> &[HostServiceExecution] {
        self.host_service_journal.executions()
    }

    pub fn grant_slots(&self) -> Vec<HostCapabilityGrant> {
        self.plan.grant_slots()
    }

    pub fn record_host_call(&mut self, call: HostCall) -> Result<(), WasmModelError> {
        let _ = self.execute_host_call(call)?;
        Ok(())
    }

    pub fn execute_host_call(
        &mut self,
        call: HostCall,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let grant = required_grant_for_host_call(&call);
        if !self.plan.granted_capabilities.contains(&grant) {
            return Err(WasmModelError::HostGrantDenied {
                handler_id: self.plan.handler_id.to_string(),
                grant,
            });
        }

        self.record_usage_for_call(&call)?;
        let service_call = host_service_call_for_host_call(&call)?;
        let execution = self
            .host_service_journal
            .execute(service_call, &self.plan.context)?;
        self.host_calls.push(call);
        Ok(execution)
    }

    pub fn reserve_concurrency(&mut self, units: u16) -> Result<(), WasmModelError> {
        self.active_concurrency = self.active_concurrency.saturating_add(units);
        self.usage.peak_concurrency = self.usage.peak_concurrency.max(self.active_concurrency);
        if self.usage.peak_concurrency > self.plan.limits.max_concurrency {
            return Err(WasmModelError::ResourceLimitExceeded {
                handler_id: self.plan.handler_id.to_string(),
                field: "max_concurrency",
            });
        }
        Ok(())
    }

    pub fn release_concurrency(&mut self, units: u16) {
        self.active_concurrency = self.active_concurrency.saturating_sub(units);
    }

    pub fn finish(
        self,
        runtime: Duration,
        outcome: InvocationOutcome,
        typed_output: Option<TypedExecutionOutput>,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        if runtime > self.plan.limits.max_runtime {
            return Err(WasmModelError::RuntimeBudgetExceeded {
                handler_id: self.plan.handler_id.to_string(),
                max_runtime: self.plan.limits.max_runtime,
                actual_runtime: runtime,
            });
        }

        let valid = matches!(
            (self.plan.point, &outcome),
            (
                crate::ids::ExtensionPointKind::Page,
                InvocationOutcome::Page
            ) | (
                crate::ids::ExtensionPointKind::Api,
                InvocationOutcome::ApiJson
            ) | (
                crate::ids::ExtensionPointKind::Job,
                InvocationOutcome::JobCompleted
            ) | (
                crate::ids::ExtensionPointKind::ScheduledJob,
                InvocationOutcome::ScheduledJobCompleted
            ) | (
                crate::ids::ExtensionPointKind::Webhook,
                InvocationOutcome::WebhookAccepted
            ) | (
                crate::ids::ExtensionPointKind::AdminWidget,
                InvocationOutcome::AdminWidget
            ) | (
                crate::ids::ExtensionPointKind::RenderHook,
                InvocationOutcome::RenderHook
            )
        );

        if !valid {
            return Err(WasmModelError::InvalidOutcomeForPoint {
                handler_id: self.plan.handler_id.to_string(),
                point: self.plan.point,
                outcome: outcome_label(&outcome),
            });
        }

        if let Some(ref typed_output) = typed_output {
            typed_output.validate_for_point(self.plan.point)?;
        }

        Ok(ExecutionReceipt {
            extension_id: self.plan.extension_id,
            handler_id: self.plan.handler_id,
            point: self.plan.point,
            runtime,
            usage: self.usage,
            outcome,
            host_calls: self.host_calls,
            host_service_executions: self.host_service_journal.executions().to_vec(),
            typed_output,
        })
    }

    fn record_usage_for_call(&mut self, call: &HostCall) -> Result<(), WasmModelError> {
        match call {
            HostCall::StorageWrite { bytes, .. } => {
                self.usage.storage_writes = self.usage.storage_writes.saturating_add(1);
                self.usage.storage_bytes = self.usage.storage_bytes.saturating_add(*bytes);
                if self.usage.storage_writes > self.plan.limits.max_storage_writes {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_storage_writes",
                    });
                }
                if self.usage.storage_bytes > self.plan.limits.max_storage_bytes {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_storage_bytes",
                    });
                }
            }
            HostCall::OutboundHttp { response_bytes, .. } => {
                self.usage.outbound_requests = self.usage.outbound_requests.saturating_add(1);
                self.usage.outbound_response_bytes = self
                    .usage
                    .outbound_response_bytes
                    .saturating_add(*response_bytes);
                if self.usage.outbound_requests > self.plan.limits.max_outbound_requests {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_outbound_requests",
                    });
                }
                if self.usage.outbound_response_bytes > self.plan.limits.max_outbound_response_bytes
                {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_outbound_response_bytes",
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }
}

fn host_service_call_for_host_call(call: &HostCall) -> Result<HostServiceCall, WasmModelError> {
    let request = match call {
        HostCall::DataRead { resource } => HostServiceRequest::Data(DataServiceRequest::Read {
            resource: resource.clone(),
        }),
        HostCall::DataWrite { resource } => HostServiceRequest::Data(DataServiceRequest::Write {
            resource: resource.clone(),
        }),
        HostCall::AuthCheck => HostServiceRequest::Auth(AuthServiceRequest::Check),
        HostCall::AuthList => HostServiceRequest::Auth(AuthServiceRequest::List),
        HostCall::AuthLookup => HostServiceRequest::Auth(AuthServiceRequest::Lookup),
        HostCall::AuthTupleWrite => HostServiceRequest::Auth(AuthServiceRequest::TupleWrite),
        HostCall::StorageRead { class } => {
            HostServiceRequest::Storage(StorageServiceRequest::Read { class: *class })
        }
        HostCall::StorageWrite { class, bytes } => {
            HostServiceRequest::Storage(StorageServiceRequest::Write {
                class: *class,
                bytes: *bytes,
            })
        }
        HostCall::RenderFragment { slot } => {
            HostServiceRequest::Render(RenderServiceRequest::Fragment { slot: slot.clone() })
        }
        HostCall::MetadataWrite { kind } => HostServiceRequest::MetadataWrite { kind: *kind },
        HostCall::CacheHintWrite => {
            HostServiceRequest::CacheIntent(CacheIntentServiceRequest::HintWrite)
        }
        HostCall::OutboundHttp {
            integration,
            response_bytes,
        } => HostServiceRequest::OutboundHttp {
            integration: integration.clone(),
            response_bytes: *response_bytes,
        },
        HostCall::SecretRead { secret } => HostServiceRequest::SecretRead {
            secret: secret.clone(),
        },
        HostCall::EnqueueJob { queue } => HostServiceRequest::EnqueueJob {
            queue: queue.clone(),
        },
    };

    Ok(HostServiceCall::new(
        required_grant_for_host_call(call),
        request,
    ))
}

fn required_grant_for_host_call(call: &HostCall) -> HostCapabilityGrant {
    match call {
        HostCall::DataRead { resource } => HostCapabilityGrant::DataRead {
            resource: resource.clone(),
        },
        HostCall::DataWrite { resource } => HostCapabilityGrant::DataWrite {
            resource: resource.clone(),
        },
        HostCall::AuthCheck => HostCapabilityGrant::AuthCheck,
        HostCall::AuthList => HostCapabilityGrant::AuthList,
        HostCall::AuthLookup => HostCapabilityGrant::AuthLookup,
        HostCall::AuthTupleWrite => HostCapabilityGrant::AuthTupleWrite,
        HostCall::StorageRead { class } => HostCapabilityGrant::StorageRead { class: *class },
        HostCall::StorageWrite { class, .. } => HostCapabilityGrant::StorageWrite { class: *class },
        HostCall::RenderFragment { slot } => {
            HostCapabilityGrant::RenderFragment { slot: slot.clone() }
        }
        HostCall::MetadataWrite { kind } => HostCapabilityGrant::MetadataWrite { kind: *kind },
        HostCall::CacheHintWrite => HostCapabilityGrant::CacheHintWrite,
        HostCall::OutboundHttp { integration, .. } => HostCapabilityGrant::OutboundHttp {
            integration: integration.clone(),
        },
        HostCall::SecretRead { secret } => HostCapabilityGrant::SecretRead {
            secret: secret.clone(),
        },
        HostCall::EnqueueJob { queue } => HostCapabilityGrant::EnqueueJob {
            queue: queue.clone(),
        },
    }
}

fn outcome_label(outcome: &InvocationOutcome) -> &'static str {
    match outcome {
        InvocationOutcome::Page => "page",
        InvocationOutcome::ApiJson => "api_json",
        InvocationOutcome::JobCompleted => "job_completed",
        InvocationOutcome::ScheduledJobCompleted => "scheduled_job_completed",
        InvocationOutcome::WebhookAccepted => "webhook_accepted",
        InvocationOutcome::AdminWidget => "admin_widget",
        InvocationOutcome::RenderHook => "render_hook",
    }
}

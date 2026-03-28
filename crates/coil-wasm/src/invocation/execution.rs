use std::sync::Arc;
use std::time::Duration;

use crate::error::WasmModelError;
use crate::grants::{
    HostCapabilityGrant, HostGrantSet, MetadataGrant, ResourceLimits, StorageClassGrant,
};
use crate::host_services::{HostServiceExecution, HostServiceExecutor, HostServiceSessionState};
use crate::ids::{ExtensionId, ExtensionPointKind, HandlerId};
use crate::output::TypedExecutionOutput;

use super::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationPlan {
    pub extension_id: ExtensionId,
    pub handler_id: HandlerId,
    pub point: ExtensionPointKind,
    pub customer_app_id: String,
    pub granted_capabilities: HostGrantSet,
    pub limits: ResourceLimits,
    pub context: InvocationContext,
}

impl InvocationPlan {
    pub fn begin_execution(self) -> WasmExecutionSession {
        WasmExecutionSession::new(self)
    }

    pub fn begin_synthetic_execution(self) -> WasmExecutionSession {
        WasmExecutionSession::with_executor(
            self,
            Arc::new(crate::host_services::SyntheticHostServiceExecutor::default()),
        )
    }

    pub fn begin_execution_with_executor(
        self,
        executor: Arc<dyn HostServiceExecutor>,
    ) -> WasmExecutionSession {
        WasmExecutionSession::with_executor(self, executor)
    }

    pub fn grant_slots(&self) -> Vec<HostCapabilityGrant> {
        self.granted_capabilities.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCall {
    DataRead {
        resource: String,
    },
    DataWrite {
        resource: String,
    },
    AuthCheck,
    AuthList,
    AuthLookup,
    AuthTupleWrite,
    StorageRead {
        class: StorageClassGrant,
    },
    StorageWrite {
        class: StorageClassGrant,
        bytes: u64,
    },
    RenderFragment {
        slot: String,
    },
    MetadataWrite {
        kind: MetadataGrant,
    },
    CacheHintWrite,
    OutboundHttp {
        integration: String,
        response_bytes: u64,
    },
    SecretRead {
        secret: String,
    },
    EnqueueJob {
        queue: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationOutcome {
    Page,
    ApiJson,
    JobCompleted,
    ScheduledJobCompleted,
    WebhookAccepted,
    AdminWidget,
    RenderHook,
}

impl InvocationOutcome {
    pub fn engine_code(&self) -> i32 {
        match self {
            Self::Page => 0,
            Self::ApiJson => 1,
            Self::JobCompleted => 2,
            Self::ScheduledJobCompleted => 3,
            Self::WebhookAccepted => 4,
            Self::AdminWidget => 5,
            Self::RenderHook => 6,
        }
    }

    pub fn from_engine_code(code: i32, handler_id: String) -> Result<Self, WasmModelError> {
        match code {
            0 => Ok(Self::Page),
            1 => Ok(Self::ApiJson),
            2 => Ok(Self::JobCompleted),
            3 => Ok(Self::ScheduledJobCompleted),
            4 => Ok(Self::WebhookAccepted),
            5 => Ok(Self::AdminWidget),
            6 => Ok(Self::RenderHook),
            _ => Err(WasmModelError::InvalidOutcomeCode { handler_id, code }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionUsage {
    pub outbound_requests: u32,
    pub outbound_response_bytes: u64,
    pub storage_writes: u32,
    pub storage_bytes: u64,
    pub peak_concurrency: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub extension_id: ExtensionId,
    pub handler_id: HandlerId,
    pub point: ExtensionPointKind,
    pub runtime: Duration,
    pub usage: ExecutionUsage,
    pub outcome: InvocationOutcome,
    pub host_calls: Vec<HostCall>,
    pub host_service_executions: Vec<HostServiceExecution>,
    pub typed_output: Option<TypedExecutionOutput>,
}

#[derive(Debug, Clone)]
pub struct WasmExecutionSession {
    state: HostServiceSessionState,
}

impl WasmExecutionSession {
    pub fn new(plan: InvocationPlan) -> Self {
        Self {
            state: HostServiceSessionState::new(plan),
        }
    }

    pub fn with_executor(plan: InvocationPlan, executor: Arc<dyn HostServiceExecutor>) -> Self {
        Self {
            state: HostServiceSessionState::with_executor(plan, executor),
        }
    }

    pub fn plan(&self) -> &InvocationPlan {
        self.state.plan()
    }

    pub fn usage(&self) -> &ExecutionUsage {
        self.state.usage()
    }

    pub fn host_calls(&self) -> &[HostCall] {
        self.state.host_calls()
    }

    pub fn host_service_executions(&self) -> &[HostServiceExecution] {
        self.state.host_service_executions()
    }

    pub fn grant_slots(&self) -> Vec<HostCapabilityGrant> {
        self.state.grant_slots()
    }

    pub fn execute_host_call(
        &mut self,
        call: HostCall,
    ) -> Result<HostServiceExecution, WasmModelError> {
        self.state.execute_host_call(call)
    }

    pub fn record_host_call(&mut self, call: HostCall) -> Result<(), WasmModelError> {
        self.state.record_host_call(call)
    }

    pub fn reserve_concurrency(&mut self, units: u16) -> Result<(), WasmModelError> {
        self.state.reserve_concurrency(units)
    }

    pub fn release_concurrency(&mut self, units: u16) {
        self.state.release_concurrency(units)
    }

    pub fn finish(
        self,
        runtime: Duration,
        outcome: InvocationOutcome,
        typed_output: Option<TypedExecutionOutput>,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        self.state.finish(runtime, outcome, typed_output)
    }
}

use crate::error::OpsModelError;
use crate::identifiers::{RecoveryExecutionId, RecoveryWorkflowId};
use crate::validation::require_non_empty;
use coil_auth::Capability;
use coil_jobs::{IdempotencyKey, JobInstant, JobSpec, PlannedJob};

use super::{RecoveryStage, RecoveryWorkflowDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPlanRequest {
    pub execution_id: RecoveryExecutionId,
    pub definition_id: RecoveryWorkflowId,
    pub customer_app_id: String,
    pub requested_by: String,
    pub requested_at: JobInstant,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub operator_capabilities: Vec<Capability>,
    pub local_only_sensitive_present: bool,
    pub local_only_sensitive_acknowledged: bool,
}

impl RecoveryPlanRequest {
    pub fn new(
        execution_id: RecoveryExecutionId,
        definition_id: RecoveryWorkflowId,
        customer_app_id: impl Into<String>,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            execution_id,
            definition_id,
            customer_app_id: require_non_empty("recovery_customer_app_id", customer_app_id.into())?,
            requested_by: require_non_empty("recovery_requested_by", requested_by.into())?,
            requested_at,
            scheduled_for: None,
            idempotency_key: None,
            operator_capabilities: Vec::new(),
            local_only_sensitive_present: false,
            local_only_sensitive_acknowledged: false,
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.operator_capabilities.push(capability);
        self
    }

    pub fn with_local_only_sensitive_data(mut self, acknowledged: bool) -> Self {
        self.local_only_sensitive_present = true;
        self.local_only_sensitive_acknowledged = acknowledged;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPlan {
    pub definition: RecoveryWorkflowDefinition,
    pub job: JobSpec,
    pub planned_job: PlannedJob,
    pub customer_app_id: String,
    pub stages: Vec<RecoveryStage>,
    pub audit_message: String,
    pub requires_host_local_restore: bool,
}

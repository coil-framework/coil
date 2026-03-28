use crate::error::OpsModelError;
use crate::identifiers::BulkOperationId;
use crate::validation::require_non_empty;
use coil_auth::Capability;
use coil_jobs::{IdempotencyKey, JobInstant, JobSpec, PlannedJob};

use super::BulkOperationDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationRequest {
    pub execution_id: crate::BulkExecutionId,
    pub definition_id: BulkOperationId,
    pub requested_by: String,
    pub requested_at: JobInstant,
    pub target_count: usize,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub operator_capabilities: Vec<Capability>,
    pub dry_run: bool,
}

impl BulkOperationRequest {
    pub fn new(
        execution_id: crate::BulkExecutionId,
        definition_id: BulkOperationId,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
        target_count: usize,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            execution_id,
            definition_id,
            requested_by: require_non_empty("bulk_requested_by", requested_by.into())?,
            requested_at,
            target_count,
            scheduled_for: None,
            idempotency_key: None,
            operator_capabilities: Vec::new(),
            dry_run: false,
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

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationPlan {
    pub definition: BulkOperationDefinition,
    pub job: JobSpec,
    pub planned_job: PlannedJob,
    pub dry_run: bool,
    pub target_count: usize,
    pub audit_message: String,
}

use crate::error::OpsModelError;
use crate::identifiers::RecoveryWorkflowId;
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_jobs::RetryPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStage {
    RestoreDatabase,
    ReattachManagedObjectStore,
    RestoreLocalOnlySensitive,
    RebuildDerivedState,
    RedeployStaticAssets,
    ValidateReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryWorkflowDefinition {
    pub id: RecoveryWorkflowId,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub retry_policy: RetryPolicy,
    pub requires_idempotency_key: bool,
    pub requires_local_only_sensitive_ack: bool,
    pub default_stages: Vec<RecoveryStage>,
}

impl RecoveryWorkflowDefinition {
    pub fn new(
        id: RecoveryWorkflowId,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        retry_policy: RetryPolicy,
        requires_idempotency_key: bool,
        requires_local_only_sensitive_ack: bool,
        default_stages: Vec<RecoveryStage>,
    ) -> Result<Self, OpsModelError> {
        let title = require_non_empty("recovery_title", title.into())?;
        if default_stages.is_empty() {
            return Err(OpsModelError::InvalidRecoveryWorkflow {
                workflow_id: id.to_string(),
                reason: "recovery workflow must declare at least one stage".to_string(),
            });
        }
        if retry_policy.is_retrying() && !requires_idempotency_key {
            return Err(OpsModelError::InvalidRecoveryWorkflow {
                workflow_id: id.to_string(),
                reason: "retrying recovery workflows must require an idempotency key".to_string(),
            });
        }

        Ok(Self {
            id,
            title,
            description,
            required_capability,
            retry_policy,
            requires_idempotency_key,
            requires_local_only_sensitive_ack,
            default_stages,
        })
    }

    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        capabilities.contains(&self.required_capability)
    }
}

use std::collections::BTreeSet;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_jobs::RetryPolicy;

use crate::error::OpsModelError;
use crate::identifiers::RecoveryWorkflowId;

use super::{RecoveryStage, RecoveryWorkflowDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCatalog {
    definitions: Vec<RecoveryWorkflowDefinition>,
}

impl RecoveryCatalog {
    pub fn new(definitions: Vec<RecoveryWorkflowDefinition>) -> Self {
        Self { definitions }
    }

    pub fn standard() -> Self {
        Self::new(vec![
            RecoveryWorkflowDefinition::new(
                RecoveryWorkflowId::new("recovery.customer-app.full-restore")
                    .expect("constant workflow id is valid"),
                "Full customer-app restore",
                Some(
                    "Restores source-of-truth state first, then rebuilds disposable layers and redeploys assets."
                        .to_string(),
                ),
                Capability::SystemModuleManage,
                default_recovery_retry_policy(),
                true,
                true,
                vec![
                    RecoveryStage::RestoreDatabase,
                    RecoveryStage::ReattachManagedObjectStore,
                    RecoveryStage::RestoreLocalOnlySensitive,
                    RecoveryStage::RebuildDerivedState,
                    RecoveryStage::RedeployStaticAssets,
                    RecoveryStage::ValidateReadiness,
                ],
            )
            .expect("constant recovery workflow is valid"),
            RecoveryWorkflowDefinition::new(
                RecoveryWorkflowId::new("recovery.customer-app.derived-state")
                    .expect("constant workflow id is valid"),
                "Derived-state rebuild",
                Some(
                    "Rebuilds caches, search indexes, report snapshots, and redeploys static assets without restoring source-of-truth storage."
                        .to_string(),
                ),
                Capability::SystemModuleManage,
                default_recovery_retry_policy(),
                true,
                false,
                vec![
                    RecoveryStage::RebuildDerivedState,
                    RecoveryStage::RedeployStaticAssets,
                    RecoveryStage::ValidateReadiness,
                ],
            )
            .expect("constant recovery workflow is valid"),
        ])
    }

    pub fn definitions(&self) -> &[RecoveryWorkflowDefinition] {
        &self.definitions
    }

    pub fn definition(&self, id: &RecoveryWorkflowId) -> Option<&RecoveryWorkflowDefinition> {
        self.definitions.iter().find(|definition| &definition.id == id)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut ids = BTreeSet::new();
        for definition in &self.definitions {
            if !ids.insert(definition.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "recovery workflow",
                    id: definition.id.to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for RecoveryCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

fn default_recovery_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(30), Duration::from_secs(900))
        .expect("constant recovery retry policy is valid")
}

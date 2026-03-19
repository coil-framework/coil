use davenda_config::{PlatformConfig, StorageClass};
use thiserror::Error;

use crate::policy::{join_local_path, join_relative, normalize_relative_path};
use crate::{
    DurableStore, StorageBackendKind, StoragePolicy, StoragePolicyError, StoragePolicyOverride,
    StoragePolicySet, StorageTopology, SyncMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlanRequest {
    pub logical_path: String,
    pub storage_class: Option<StorageClass>,
    pub override_policy: Option<StoragePolicyOverride>,
}

impl StoragePlanRequest {
    pub fn new(logical_path: impl Into<String>) -> Self {
        Self {
            logical_path: logical_path.into(),
            storage_class: None,
            override_policy: None,
        }
    }

    pub fn with_storage_class(mut self, storage_class: StorageClass) -> Self {
        self.storage_class = Some(storage_class);
        self
    }

    pub fn with_override(mut self, override_policy: StoragePolicyOverride) -> Self {
        self.override_policy = Some(override_policy);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTargetKind {
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteTarget {
    pub backend: StorageBackendKind,
    pub locator: String,
    pub kind: WriteTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageDeploymentScope {
    Scalable,
    SingleNodeOnly,
}

impl StorageDeploymentScope {
    pub const fn requires_single_node(self) -> bool {
        matches!(self, Self::SingleNodeOnly)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlan {
    pub logical_path: String,
    pub storage_class: StorageClass,
    pub policy: StoragePolicy,
    pub durable_store: DurableStore,
    pub object_key: Option<String>,
    pub local_path: Option<String>,
    pub matched_rule_prefix: Option<String>,
    pub write_targets: Vec<WriteTarget>,
    pub deployment_scope: StorageDeploymentScope,
}

impl StoragePlan {
    pub fn primary_write_target(&self) -> Option<&WriteTarget> {
        self.write_targets
            .iter()
            .find(|target| target.kind == WriteTargetKind::Primary)
    }

    pub fn ensure_public_delivery_allowed(&self) -> Result<(), StoragePlanningError> {
        if self.policy.is_public_delivery_eligible() {
            Ok(())
        } else {
            Err(StoragePlanningError::PublicDeliveryNotEligible {
                logical_path: self.logical_path.clone(),
                policy: self.policy,
            })
        }
    }

    pub const fn public_delivery_eligible(&self) -> bool {
        self.policy.is_public_delivery_eligible()
    }

    pub const fn requires_single_node(&self) -> bool {
        self.deployment_scope.requires_single_node()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlanner {
    topology: StorageTopology,
    policies: StoragePolicySet,
}

impl StoragePlanner {
    pub fn from_config(config: &PlatformConfig) -> Self {
        Self {
            topology: StorageTopology::from_config(config),
            policies: StoragePolicySet::default(),
        }
    }

    pub fn new(topology: StorageTopology, policies: StoragePolicySet) -> Self {
        Self { topology, policies }
    }

    pub fn topology(&self) -> &StorageTopology {
        &self.topology
    }

    pub fn policies(&self) -> &StoragePolicySet {
        &self.policies
    }

    pub fn plan_scalable_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, StoragePlanningError> {
        let logical_path = normalize_relative_path(&request.logical_path)?;
        let storage_class = request.storage_class.unwrap_or(self.topology.default_class);
        let resolved = self.policies.resolve(
            storage_class,
            &logical_path,
            request.override_policy.as_ref(),
        )?;

        self.plan_resolved_scalable_write(logical_path, resolved)
    }

    pub fn plan_single_node_escape_hatch_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, StoragePlanningError> {
        let logical_path = normalize_relative_path(&request.logical_path)?;
        let storage_class = request.storage_class.unwrap_or(self.topology.default_class);
        let resolved = self.policies.resolve(
            storage_class,
            &logical_path,
            request.override_policy.as_ref(),
        )?;

        self.plan_resolved_single_node_escape_hatch_write(logical_path, resolved)
    }

    pub fn plan_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, StoragePlanningError> {
        self.plan_scalable_write(request.clone()).or_else(|error| match error {
            StoragePlanningError::SingleNodeEscapeHatchRequested { .. } => {
                self.plan_single_node_escape_hatch_write(request)
            }
            other => Err(other),
        })
    }

    fn plan_resolved_scalable_write(
        &self,
        logical_path: String,
        resolved: crate::ResolvedStoragePolicy,
    ) -> Result<StoragePlan, StoragePlanningError> {
        let policy = resolved.policy;
        if policy.sync_mode == SyncMode::LocalOnly {
            return Err(StoragePlanningError::SingleNodeEscapeHatchRequested {
                logical_path,
                policy,
            });
        }

        let durable_store = policy.durable_store();
        let object_key = Some(join_relative(
            resolved.object_prefix.as_deref(),
            &logical_path,
        ));

        if self.topology.object_store.is_none() {
            return Err(StoragePlanningError::ObjectStoreRequired {
                logical_path,
                policy,
            });
        }

        let write_targets = vec![WriteTarget {
            backend: self
                .topology
                .object_store
                .as_ref()
                .expect("object store availability checked")
                .backend_kind(),
            locator: object_key
                .clone()
                .expect("object key is present for object store policies"),
            kind: WriteTargetKind::Primary,
        }];

        Ok(StoragePlan {
            logical_path,
            storage_class: resolved.storage_class,
            policy,
            durable_store,
            object_key,
            local_path: None,
            matched_rule_prefix: resolved.matched_rule_prefix,
            write_targets,
            deployment_scope: StorageDeploymentScope::Scalable,
        })
    }

    fn plan_resolved_single_node_escape_hatch_write(
        &self,
        logical_path: String,
        resolved: crate::ResolvedStoragePolicy,
    ) -> Result<StoragePlan, StoragePlanningError> {
        let policy = resolved.policy;
        if policy.sync_mode != SyncMode::LocalOnly {
            return self.plan_resolved_scalable_write(logical_path, resolved);
        }

        if !self.topology.allows_explicit_local_only() {
            return Err(StoragePlanningError::SingleNodeEscapeHatchNotAllowedForDeployment {
                logical_path,
                policy,
                deployment: self.topology.deployment,
                single_node_escape_hatch: self.topology.single_node_escape_hatch,
            });
        }

        let local_path = Some(join_local_path(
            &self.topology.local_root,
            resolved.local_subdir.as_deref(),
            &logical_path,
        ));
        let write_targets = vec![WriteTarget {
            backend: StorageBackendKind::LocalDisk,
            locator: local_path
                .clone()
                .expect("local path is present for local policies"),
            kind: WriteTargetKind::Primary,
        }];

        Ok(StoragePlan {
            logical_path,
            storage_class: resolved.storage_class,
            policy,
            durable_store: policy.durable_store(),
            object_key: None,
            local_path,
            matched_rule_prefix: resolved.matched_rule_prefix,
            write_targets,
            deployment_scope: StorageDeploymentScope::SingleNodeOnly,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoragePlanningError {
    #[error(transparent)]
    Policy(#[from] StoragePolicyError),
    #[error(
        "storage plan for `{logical_path}` is not eligible for public delivery with policy {policy:?}"
    )]
    PublicDeliveryNotEligible {
        logical_path: String,
        policy: StoragePolicy,
    },
    #[error(
        "storage plan for `{logical_path}` with policy {policy:?} requires the single-node escape hatch and must use the explicit escape-hatch planner"
    )]
    SingleNodeEscapeHatchRequested {
        logical_path: String,
        policy: StoragePolicy,
    },
    #[error(
        "storage plan for `{logical_path}` requires object storage but no object-store backend is configured for policy {policy:?}"
    )]
    ObjectStoreRequired {
        logical_path: String,
        policy: StoragePolicy,
    },
    #[error(
        "storage plan for `{logical_path}` with policy {policy:?} is not allowed for deployment {deployment:?} because the single-node escape hatch is {single_node_escape_hatch:?}"
    )]
    SingleNodeEscapeHatchNotAllowedForDeployment {
        logical_path: String,
        policy: StoragePolicy,
        deployment: davenda_config::StorageDeployment,
        single_node_escape_hatch: davenda_config::SingleNodeStorageMode,
    },
}

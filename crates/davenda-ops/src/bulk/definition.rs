use crate::error::OpsModelError;
use crate::identifiers::BulkOperationId;
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    BulkOperationDefinition as ManifestBulkOperationDefinition,
    BulkOperationKind as ManifestBulkOperationKind,
    BulkOperationScope as ManifestBulkOperationScope,
};
use davenda_jobs::RetryPolicy;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationKind {
    Publish,
    Unpublish,
    Reindex,
    Export,
    Cancel,
    CheckIn,
    Custom,
}

impl fmt::Display for BulkOperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publish => f.write_str("publish"),
            Self::Unpublish => f.write_str("unpublish"),
            Self::Reindex => f.write_str("reindex"),
            Self::Export => f.write_str("export"),
            Self::Cancel => f.write_str("cancel"),
            Self::CheckIn => f.write_str("check_in"),
            Self::Custom => f.write_str("custom"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationScope {
    Cms,
    Commerce,
    Memberships,
    Events,
    Media,
    Search,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationDefinition {
    pub id: BulkOperationId,
    pub source_module: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub kind: BulkOperationKind,
    pub scope: BulkOperationScope,
    pub retry_policy: RetryPolicy,
    pub max_items: Option<usize>,
    pub requires_idempotency_key: bool,
}

impl BulkOperationDefinition {
    pub fn new(
        id: BulkOperationId,
        source_module: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        kind: BulkOperationKind,
        scope: BulkOperationScope,
        retry_policy: RetryPolicy,
        max_items: Option<usize>,
        requires_idempotency_key: bool,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("bulk_source_module", source_module.into())?;
        let title = require_non_empty("bulk_title", title.into())?;

        if let Some(max_items) = max_items {
            if max_items == 0 {
                return Err(OpsModelError::InvalidBulkOperation {
                    operation_id: id.to_string(),
                    reason: "max_items must be greater than zero".to_string(),
                });
            }
        }

        if retry_policy.is_retrying() && !requires_idempotency_key {
            return Err(OpsModelError::InvalidBulkOperation {
                operation_id: id.to_string(),
                reason: "retrying bulk operations must require an idempotency key".to_string(),
            });
        }

        Ok(Self {
            id,
            source_module,
            title,
            description,
            required_capability,
            kind,
            scope,
            retry_policy,
            max_items,
            requires_idempotency_key,
        })
    }

    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        capabilities.contains(&self.required_capability)
    }

    pub(crate) fn from_manifest_definition(
        source_module: &str,
        definition: &ManifestBulkOperationDefinition,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            BulkOperationId::new(definition.id.clone())?,
            source_module,
            definition.title.clone(),
            definition.description.clone(),
            definition.required_capability,
            map_bulk_operation_kind(definition.kind),
            map_bulk_operation_scope(definition.scope),
            definition.retry_policy.clone(),
            definition.max_items,
            definition.requires_idempotency_key,
        )
    }
}

pub(crate) fn map_bulk_operation_kind(kind: ManifestBulkOperationKind) -> BulkOperationKind {
    match kind {
        ManifestBulkOperationKind::Publish => BulkOperationKind::Publish,
        ManifestBulkOperationKind::Unpublish => BulkOperationKind::Unpublish,
        ManifestBulkOperationKind::Reindex => BulkOperationKind::Reindex,
        ManifestBulkOperationKind::Export => BulkOperationKind::Export,
        ManifestBulkOperationKind::Cancel => BulkOperationKind::Cancel,
        ManifestBulkOperationKind::CheckIn => BulkOperationKind::CheckIn,
        ManifestBulkOperationKind::Custom => BulkOperationKind::Custom,
    }
}

pub(crate) fn map_bulk_operation_scope(scope: ManifestBulkOperationScope) -> BulkOperationScope {
    match scope {
        ManifestBulkOperationScope::Cms => BulkOperationScope::Cms,
        ManifestBulkOperationScope::Commerce => BulkOperationScope::Commerce,
        ManifestBulkOperationScope::Memberships => BulkOperationScope::Memberships,
        ManifestBulkOperationScope::Events => BulkOperationScope::Events,
        ManifestBulkOperationScope::Media => BulkOperationScope::Media,
        ManifestBulkOperationScope::Search => BulkOperationScope::Search,
        ManifestBulkOperationScope::System => BulkOperationScope::System,
    }
}

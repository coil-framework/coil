use crate::error::OpsModelError;
use crate::identifiers::BulkOperationId;
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    BulkOperationDefinition as ManifestBulkOperationDefinition,
    BulkOperationKind as ManifestBulkOperationKind,
    BulkOperationScope as ManifestBulkOperationScope, ModuleManifest,
};
use davenda_jobs::{IdempotencyKey, JobInstant, JobSpec, PlannedJob, RetryPolicy};
use std::collections::HashSet;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkCatalog {
    pub definitions: Vec<BulkOperationDefinition>,
}

impl BulkCatalog {
    pub fn new(definitions: Vec<BulkOperationDefinition>) -> Self {
        Self { definitions }
    }

    pub fn standard() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut definitions = Vec::new();
        for manifest in manifests {
            for definition in &manifest.bulk_operations {
                definitions.push(BulkOperationDefinition::from_manifest_definition(
                    &manifest.name,
                    definition,
                )?);
            }
        }
        let catalog = Self::new(definitions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for definition in &self.definitions {
            if !seen.insert(definition.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "bulk operation",
                    id: definition.id.to_string(),
                });
            }

            BulkOperationDefinition::new(
                definition.id.clone(),
                definition.source_module.clone(),
                definition.title.clone(),
                definition.description.clone(),
                definition.required_capability,
                definition.kind,
                definition.scope,
                definition.retry_policy.clone(),
                definition.max_items,
                definition.requires_idempotency_key,
            )?;
        }

        Ok(())
    }

    pub fn definition(&self, id: &BulkOperationId) -> Option<&BulkOperationDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.id == id)
    }
}

impl Default for BulkCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

fn map_bulk_operation_kind(kind: ManifestBulkOperationKind) -> BulkOperationKind {
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

fn map_bulk_operation_scope(scope: ManifestBulkOperationScope) -> BulkOperationScope {
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

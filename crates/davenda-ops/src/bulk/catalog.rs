use std::collections::HashSet;

use davenda_core::ModuleManifest;

use crate::error::OpsModelError;
use crate::identifiers::BulkOperationId;

use super::BulkOperationDefinition;

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

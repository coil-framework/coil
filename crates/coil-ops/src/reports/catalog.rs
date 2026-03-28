use std::collections::HashSet;

use coil_core::ModuleManifest;

use crate::error::OpsModelError;
use crate::identifiers::ReportId;

use super::ReportDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportCatalog {
    pub definitions: Vec<ReportDefinition>,
}

impl ReportCatalog {
    pub fn new(definitions: Vec<ReportDefinition>) -> Self {
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
            for definition in &manifest.report_definitions {
                definitions.push(ReportDefinition::from_manifest_definition(
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
                    kind: "report",
                    id: definition.id.to_string(),
                });
            }

            ReportDefinition::new(
                definition.id.clone(),
                definition.source_module.clone(),
                definition.title.clone(),
                definition.description.clone(),
                definition.required_capability,
                definition.format,
                definition.sensitivity,
                definition.delivery_mode,
                definition.export_prefix.clone(),
                definition.retry_policy.clone(),
            )?;
        }

        Ok(())
    }

    pub fn definition(&self, id: &ReportId) -> Option<&ReportDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.id == id)
    }
}

impl Default for ReportCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

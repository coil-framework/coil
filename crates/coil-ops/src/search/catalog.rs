use super::*;
use coil_core::ModuleManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCatalog {
    pub contributions: Vec<SearchIndexContribution>,
}

impl SearchCatalog {
    pub fn new(contributions: Vec<SearchIndexContribution>) -> Self {
        Self { contributions }
    }

    pub fn standard() -> Self {
        Self {
            contributions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut contributions = Vec::new();
        for manifest in manifests {
            for contribution in &manifest.search_contributions {
                contributions.push(SearchIndexContribution::from_manifest_contribution(
                    &manifest.name,
                    contribution,
                )?);
            }
        }
        let catalog = Self::new(contributions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for contribution in &self.contributions {
            if !seen.insert(contribution.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "search index",
                    id: contribution.id.to_string(),
                });
            }
            SearchIndexContribution::new(
                contribution.id.clone(),
                contribution.source_module.clone(),
                contribution.document_kind,
                contribution.visibility,
                contribution.publication_required,
                contribution.fields.clone(),
                contribution.invalidation_rules.clone(),
                contribution.rebuild_strategy,
            )?;
        }

        Ok(())
    }

    pub fn contribution(&self, id: &SearchIndexId) -> Option<&SearchIndexContribution> {
        self.contributions
            .iter()
            .find(|contribution| &contribution.id == id)
    }

    pub fn visible_to(&self, capabilities: &[Capability]) -> Vec<&SearchIndexContribution> {
        self.contributions
            .iter()
            .filter(|contribution| contribution.visible_to(capabilities))
            .collect()
    }
}

impl Default for SearchCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

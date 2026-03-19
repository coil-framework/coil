use crate::bulk::BulkCatalog;
use crate::reports::ReportCatalog;
use crate::search::SearchCatalog;
use crate::OpsModelError;
use davenda_core::ModuleManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsCatalog {
    pub search: SearchCatalog,
    pub reports: ReportCatalog,
    pub bulk: BulkCatalog,
}

impl OpsCatalog {
    pub fn new(search: SearchCatalog, reports: ReportCatalog, bulk: BulkCatalog) -> Self {
        Self {
            search,
            reports,
            bulk,
        }
    }

    pub fn standard() -> Self {
        Self {
            search: SearchCatalog::standard(),
            reports: ReportCatalog::standard(),
            bulk: BulkCatalog::standard(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let catalog = Self {
            search: SearchCatalog::from_manifests(manifests)?,
            reports: ReportCatalog::from_manifests(manifests)?,
            bulk: BulkCatalog::from_manifests(manifests)?,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        self.search.validate()?;
        self.reports.validate()?;
        self.bulk.validate()?;
        Ok(())
    }
}

impl Default for OpsCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

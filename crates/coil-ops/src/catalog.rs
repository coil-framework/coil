use crate::OpsModelError;
use crate::bulk::BulkCatalog;
use crate::recovery::RecoveryCatalog;
use crate::reports::ReportCatalog;
use crate::search::SearchCatalog;
use coil_core::ModuleManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsCatalog {
    pub search: SearchCatalog,
    pub reports: ReportCatalog,
    pub bulk: BulkCatalog,
    pub recovery: RecoveryCatalog,
}

impl OpsCatalog {
    pub fn new(
        search: SearchCatalog,
        reports: ReportCatalog,
        bulk: BulkCatalog,
        recovery: RecoveryCatalog,
    ) -> Self {
        Self {
            search,
            reports,
            bulk,
            recovery,
        }
    }

    pub fn standard() -> Self {
        Self {
            search: SearchCatalog::standard(),
            reports: ReportCatalog::standard(),
            bulk: BulkCatalog::standard(),
            recovery: RecoveryCatalog::standard(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let catalog = Self {
            search: SearchCatalog::from_manifests(manifests)?,
            reports: ReportCatalog::from_manifests(manifests)?,
            bulk: BulkCatalog::from_manifests(manifests)?,
            recovery: RecoveryCatalog::standard(),
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        self.search.validate()?;
        self.reports.validate()?;
        self.bulk.validate()?;
        self.recovery.validate()?;
        Ok(())
    }
}

impl Default for OpsCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

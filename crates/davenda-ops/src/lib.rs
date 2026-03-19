use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition as ManifestBulkOperationDefinition,
    BulkOperationKind as ManifestBulkOperationKind,
    BulkOperationScope as ManifestBulkOperationScope, CapabilityContract, CoreServiceDependency,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, HttpSurfaceMethod, IntegrationKind, IntegrationPoint, JobContract,
    JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest,
    PlatformModule, RegistrationError, ReportDefinition as ManifestReportDefinition,
    ReportDeliveryMode as ManifestReportDeliveryMode, ReportFormat as ManifestReportFormat,
    ReportSensitivity as ManifestReportSensitivity, RouteSurface, RouteSurfaceKind,
    SearchDocumentKind as ManifestSearchDocumentKind,
    SearchFieldContribution as ManifestSearchFieldContribution,
    SearchFieldRole as ManifestSearchFieldRole,
    SearchIndexContribution as ManifestSearchIndexContribution,
    SearchInvalidationRule as ManifestSearchInvalidationRule,
    SearchInvalidationTrigger as ManifestSearchInvalidationTrigger,
    SearchRebuildStrategy as ManifestSearchRebuildStrategy,
    SearchVisibility as ManifestSearchVisibility, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};
use davenda_jobs::{IdempotencyKey, JobInstant, JobsRuntime};

mod bulk;
mod catalog;
mod error;
mod identifiers;
mod planner;
mod reports;
mod search;
mod validation;

pub use bulk::{
    BulkCatalog, BulkOperationDefinition, BulkOperationKind, BulkOperationPlan,
    BulkOperationRequest, BulkOperationScope,
};
pub use catalog::OpsCatalog;
pub use error::OpsModelError;
pub use identifiers::{
    BulkExecutionId, BulkOperationId, ReportExportId, ReportId, SearchFieldId, SearchIndexId,
};
pub(crate) use planner::default_retry_policy;
pub use planner::OpsPlanner;
pub use reports::{
    ReportCatalog, ReportDefinition, ReportDeliveryMode, ReportExportPlan, ReportExportRequest,
    ReportFormat, ReportParameter, ReportSensitivity,
};
pub use search::{
    SearchCatalog, SearchDocumentKind, SearchFieldContribution, SearchFieldRole,
    SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility,
};

mod module;
pub use module::OpsModule;

#[cfg(test)]
mod tests;

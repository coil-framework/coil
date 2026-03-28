use super::*;
use coil_auth::Capability;
use coil_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution, CapabilityContract,
    CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind,
    HttpSurfaceArea, HttpSurfaceContribution, HttpSurfaceMethod, IntegrationKind, IntegrationPoint,
    JobContract, JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency,
    ModuleManifest, PlatformModule, RegistrationError, ReportDefinition, ReportDeliveryMode,
    ReportFormat, ReportSensitivity, RouteSurface, RouteSurfaceKind, SearchDocumentKind,
    SearchFieldContribution, SearchFieldRole, SearchIndexContribution, SearchInvalidationRule,
    SearchInvalidationTrigger, SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use coil_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

mod core;
mod manifest;
mod migrations;
mod registration;

pub use core::MembershipsModule;

use manifest::build_manifest;
use migrations::install_module_migration_plan;
use registration::register_module_services;

impl PlatformModule for MembershipsModule {
    fn manifest(&self) -> ModuleManifest {
        build_manifest(self)
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        register_module_services(self, registry)
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        install_module_migration_plan(self)
    }
}

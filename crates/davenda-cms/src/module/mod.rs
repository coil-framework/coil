use super::*;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition, BulkOperationKind, BulkOperationScope, CapabilityContract,
    CoreServiceDependency, DataRepositoryContribution, DataRepositoryQueryProfile,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution,
    SearchFieldRole, SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use davenda_data::{
    FilterOperator, MigrationId, MigrationOwner, MigrationPlan, MigrationStep, PageRequest,
    PublicationVisibility, QueryCacheScope, QueryContext, QueryField, QueryFilter, QuerySort,
    QuerySpec, RepositorySpec, TableName,
};
use davenda_jobs::RetryPolicy;

mod core;
mod platform;
mod support;

pub use core::CmsModule;

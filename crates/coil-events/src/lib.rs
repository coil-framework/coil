use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::time::Duration;

use coil_auth::{
    Capability, DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation,
};
use coil_commerce::OrderId;
use coil_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition, BulkOperationKind, BulkOperationScope, CapabilityContract,
    CoreServiceDependency, DataRepositoryContribution, DataRepositoryQueryProfile,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, HttpSurfaceMethod, IntegrationKind, IntegrationPoint, JobContract,
    JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest,
    PlatformModule, RegistrationError, ReportDefinition, ReportDeliveryMode, ReportFormat,
    ReportSensitivity, RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution,
    SearchFieldRole, SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use coil_data::{
    DataModelError, DomainWrite, FilterOperator, MigrationId, MigrationOwner, MigrationPlan,
    MigrationStep, PageRequest, PublicationVisibility, QueryCacheScope, QueryContext, QueryField,
    QueryFilter, QuerySort, QuerySpec, RepositorySpec, TableName, TransactionIsolation,
    TransactionPlan,
};
use coil_jobs::RetryPolicy;
use coil_memberships::MembershipTierId;

mod booking;
mod catalog;
mod model;
mod module;
#[cfg(test)]
mod tests;

pub use booking::*;
pub use catalog::*;
pub use model::*;
pub use module::EventsModule;

pub fn module() -> EventsModule {
    EventsModule::new()
}

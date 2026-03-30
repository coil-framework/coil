use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Duration;

use coil_auth::{AuthModelPackage, AuthModelPackageSelection};
use coil_cache::{
    ApplicationCachePolicy, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError,
    CacheNamespace, CachePlan, CachePlanRequest, CachePlanner, CacheRuntime, CacheScope,
    CacheTopology, EntityTag, FillDecision, FreshnessPolicy, HttpCachePolicy, InvalidationSet,
    InvalidationTag, ResponseValidators,
};
use coil_config::{ConfigError, PlatformConfig};
use coil_core::{
    BrowserSecurityServices, BulkOperationDefinition, CapabilityValidationError,
    CliRuntimeServices, DataRepositoryContribution, DataRuntimeServices, EventSubscription,
    HttpFileDeliveryMode, HttpResponseContract, HttpSurfaceArea, HttpSurfaceContribution,
    HttpSurfaceMethod, I18nRuntimeServices, JobContract, JobTriggerKind, JobsRuntimeServices,
    ModuleInstallationError, ModuleManifest, ObservabilityRuntimeServices, PlatformModule,
    RegistrationError, ReportDefinition, SearchIndexContribution, SeoRuntimeServices,
    ServiceDescriptor, TemplateRuntimeServices, TlsRuntimeServices, WasmRuntimeServices,
    validate_module_capabilities, validate_module_installation,
};
use coil_data::{DataModelError, MigrationPlan};
use coil_jobs::{
    DeadLetterReason, DomainEventEnvelope, DomainEventId, DomainEventType, EventHandlerId,
    EventHandlerMetadata, EventSubscriptionId, EventSubscriptionMetadata, IdempotencyKey,
    JobFailureDisposition, JobId, JobInstant, JobLease, JobName, JobQueueName, JobSpec,
    JobsCoordinator, JobsDomain, JobsModelError, QueueTopology, RetryPolicy, SchedulerLeadership,
};
use coil_observability::{
    BrandId, CustomerAppId, FeatureFlag, FeatureFlagContext, FeatureFlagId, MaintenanceMode,
    ObservabilityError, SiteId,
};
use coil_ops::{
    BulkExecutionId, BulkOperationId, BulkOperationPlan, BulkOperationRequest, OpsCatalog,
    OpsModelError, OpsPlanner, ReportExportPlan, ReportExportRequest, SearchCatalog,
    SearchIndexContribution as OpsSearchIndexContribution, SearchInvalidationTrigger,
    SearchRebuildStrategy,
};
use coil_storage::{
    PathPolicyRule, StoragePlanRequest, StoragePlanner, StoragePolicySet, StorageTopology,
};
use coil_tls::{
    CertificateId, CertificateInventory, CertificateProviderKind, CertificateRecord,
    ChallengeTicket, EdgeMode, HotReloadEvent, IssuancePlan, RenewalPlan, TlsControlPlaneRuntime,
    TlsInstant, TlsModelError,
};
use coil_wasm::{
    AdminWidgetInvocation, ApiInvocation, CompiledWasmModule, ContractVersion, CustomerAppContext,
    ExecutionReceipt, ExtensionPointKind, ExtensionRegistry, HttpMethod as WasmHttpMethod,
    InstalledExtension, InvocationContext, InvocationInput, InvocationPlan, JobInvocation,
    PageInvocation, PrincipalRef, RenderHookInvocation, ScheduledJobInvocation, TraceContext,
    TypedCacheHint, TypedExecutionOutput, TypedMetadata, WasmEngine, WasmExecutionSession,
    WasmModelError, WebhookInvocation,
};
use thiserror::Error;

mod admin_audit;
mod backends;
mod browser;
mod builder;
mod cache;
mod cms_admin;
mod http;
mod jobs;
mod live;
mod ops;
mod plan;
mod render;
mod server;
mod storage;
mod storefront;
mod tls;
mod wasm;
mod wasm_data;

pub(crate) use admin_audit::*;
pub use browser::*;
pub use builder::*;
pub use cache::*;
pub(crate) use cms_admin::*;
pub use http::*;
pub use jobs::*;
pub(crate) use live::*;
pub use ops::*;
pub use plan::*;
pub use render::*;
pub use server::*;
pub use storage::*;
pub use storefront::*;
pub use tls::*;
pub use wasm::*;

#[cfg(test)]
mod tests;

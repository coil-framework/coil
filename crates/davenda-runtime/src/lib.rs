use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Duration;

use davenda_assets::{
    ActiveAssetManifest, AssetDeliveryPlan, AssetModelError, ContentFingerprint, DeliveryContext,
    DeploymentRelease, ManagedAsset, ManagedAssetRevision, RevisionId,
};
use davenda_auth::AuthModelPackage;
use davenda_cache::{
    ApplicationCachePolicy, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError,
    CacheNamespace, CachePlan, CachePlanRequest, CachePlanner, CacheRuntime, CacheScope,
    CacheTopology, EntityTag, FillDecision, FreshnessPolicy, HttpCachePolicy, InvalidationSet,
    InvalidationTag, ResponseValidators,
};
use davenda_config::{ConfigError, PlatformConfig};
use davenda_core::{
    BrowserSecurityServices, BulkOperationDefinition, CapabilityValidationError,
    CliRuntimeServices, DataRepositoryContribution, DataRuntimeServices, EventSubscription,
    HttpFileDeliveryMode, HttpResponseContract, HttpSurfaceArea, HttpSurfaceContribution,
    HttpSurfaceMethod, I18nRuntimeServices, JobContract, JobTriggerKind, JobsRuntimeServices,
    ModuleInstallationError, ModuleManifest, ObservabilityRuntimeServices, PlatformModule,
    RegistrationError, ReportDefinition, SearchIndexContribution, SeoRuntimeServices,
    ServiceDescriptor, TemplateRuntimeServices, TlsRuntimeServices, WasmRuntimeServices,
    bootstrap_core_services, validate_module_capabilities, validate_module_installation,
};
use davenda_data::{DataModelError, MigrationPlan};
use davenda_jobs::{
    DeadLetterReason, DomainEventEnvelope, DomainEventId, DomainEventType, EventHandlerId,
    EventHandlerMetadata, EventSubscriptionId, EventSubscriptionMetadata, IdempotencyKey,
    JobFailureDisposition, JobId, JobInstant, JobLease, JobName, JobQueueName, JobSpec,
    JobsCoordinator, JobsDomain, JobsModelError, QueueTopology, RetryPolicy, SchedulerLeadership,
};
use davenda_observability::{
    CustomerAppId, FeatureFlag, FeatureFlagContext, FeatureFlagId, MaintenanceMode,
    ObservabilityError,
};
use davenda_ops::{
    BulkExecutionId, BulkOperationId, BulkOperationPlan, BulkOperationRequest, OpsCatalog,
    OpsModelError, OpsPlanner, ReportExportPlan, ReportExportRequest, SearchCatalog,
    SearchIndexContribution as OpsSearchIndexContribution, SearchInvalidationTrigger,
    SearchRebuildStrategy,
};
use davenda_storage::{
    PathPolicyRule, StoragePlan, StoragePlanRequest, StoragePlanner, StoragePlanningError,
    StoragePolicyOverride, StoragePolicySet, StorageTopology,
};
use davenda_tls::{
    CertificateId, CertificateInventory, CertificateProviderKind, CertificateRecord,
    ChallengeTicket, EdgeMode, HostnameBinding, HotReloadEvent, IssuancePlan, RenewalPlan,
    TlsAutomationRuntime, TlsInstant, TlsModelError,
};
use davenda_wasm::{
    AdminWidgetInvocation, ApiInvocation, CacheVisibility, CompiledWasmModule, ContractVersion,
    CustomerAppContext, ExecutionReceipt, ExtensionPointKind, ExtensionRegistry,
    HttpMethod as WasmHttpMethod, InstalledExtension, InvocationContext, InvocationInput,
    InvocationPlan, JobInvocation, PageInvocation, PrincipalRef, RenderHookInvocation,
    ScheduledJobInvocation, TraceContext, TypedCacheHint, TypedExecutionOutput, TypedMetadata,
    WasmEngine, WasmExecutionSession, WasmModelError, WebhookInvocation,
};
use thiserror::Error;

mod backends;
mod browser;
mod builder;
mod cache;
mod http;
mod jobs;
mod live;
mod ops;
mod plan;
mod render;
mod server;
mod storage;
mod tls;
mod wasm;
mod wasm_data;

pub use browser::*;
pub use builder::*;
pub use cache::*;
pub use http::*;
pub use jobs::*;
pub(crate) use live::*;
pub use ops::*;
pub use plan::*;
pub use render::*;
pub use server::*;
pub use storage::*;
pub use tls::*;
pub use wasm::*;

#[cfg(test)]
mod tests;

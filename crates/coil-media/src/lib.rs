#[cfg(test)]
use coil_assets::{AssetId, ContentFingerprint};
use coil_auth::Capability;
#[cfg(test)]
use coil_auth::{DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation};
use coil_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution, CapabilityContract,
    CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind,
    HttpFileDeliveryMode, HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind,
    IntegrationPoint, JobContract, JobTriggerKind, MigrationContract, ModuleBehavior,
    ModuleDependency, ModuleManifest, PlatformModule, RegistrationError, RouteSurface,
    RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution, SearchFieldRole,
    SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use coil_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};
#[cfg(test)]
use coil_storage::{DeliveryMode, Sensitivity, StoragePolicy, StoragePolicyOverride};

mod asset;
mod error;
mod identifiers;
mod library;
mod model;
mod module;
mod validation;

pub use asset::{MediaAsset, MediaAssetRevision, PublicationState};
pub use error::MediaModelError;
pub use identifiers::{
    MediaAssetId, MediaDerivativeId, MediaFieldId, MediaFolderId, MediaLibraryId, MediaRevisionId,
    MediaSlug, MediaTag,
};
pub use library::{
    MediaAccessGrant, MediaAccessKind, MediaDerivativeHint, MediaFolder, MediaLibrary,
    MediaLibraryOverview, MediaLibraryPolicy,
};
pub use model::{
    MediaDerivative, MediaDerivativeKind, MediaMetadata, MediaTechnicalMetadata, PublicationStatus,
};
pub use module::MediaModule;

pub fn module() -> MediaModule {
    MediaModule::new()
}

#[cfg(test)]
mod tests;

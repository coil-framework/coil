use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use davenda_assets::{AssetId, ContentFingerprint};
use davenda_auth::{
    Capability, DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation,
};
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, HttpFileDeliveryMode, HttpSurfaceArea, HttpSurfaceContribution,
    IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind, MigrationContract,
    ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule, RegistrationError,
    RouteSurface, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};
use davenda_storage::{
    DeliveryMode, Sensitivity, StoragePolicy, StoragePolicyError, StoragePolicyOverride,
};

#[derive(Debug, PartialEq, Eq)]
pub enum MediaModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateIdentifier {
        kind: &'static str,
        id: String,
    },
    MissingLibrary {
        library_id: String,
    },
    MissingFolder {
        folder_id: String,
    },
    MissingAsset {
        asset_id: String,
    },
    MissingLiveRevision {
        asset_id: String,
    },
    MissingStagedReplacement {
        asset_id: String,
    },
    InvalidRevisionTransition {
        asset_id: String,
        from: PublicationStatus,
        to: PublicationStatus,
    },
    StoragePolicy {
        error: StoragePolicyError,
    },
}

impl fmt::Display for MediaModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => {
                write!(f, "{kind} `{id}` is duplicated")
            }
            Self::MissingLibrary { library_id } => {
                write!(f, "media library `{library_id}` was not found")
            }
            Self::MissingFolder { folder_id } => {
                write!(f, "media folder `{folder_id}` was not found")
            }
            Self::MissingAsset { asset_id } => {
                write!(f, "media asset `{asset_id}` was not found")
            }
            Self::MissingLiveRevision { asset_id } => {
                write!(f, "media asset `{asset_id}` has no live revision")
            }
            Self::MissingStagedReplacement { asset_id } => {
                write!(f, "media asset `{asset_id}` has no staged replacement")
            }
            Self::InvalidRevisionTransition {
                asset_id,
                from,
                to,
            } => write!(
                f,
                "media asset `{asset_id}` cannot transition from `{from}` to `{to}`"
            ),
            Self::StoragePolicy { error } => write!(f, "{error}"),
        }
    }
}

impl Error for MediaModelError {}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, MediaModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(MediaLibraryId, "media_library_id");
token_type!(MediaFolderId, "media_folder_id");
token_type!(MediaAssetId, "media_asset_id");
token_type!(MediaRevisionId, "media_revision_id");
token_type!(MediaDerivativeId, "media_derivative_id");
token_type!(MediaSlug, "media_slug");
token_type!(MediaTag, "media_tag");
token_type!(MediaFieldId, "media_field_id");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStatus {
    Draft,
    Published,
    Unpublished,
    Archived,
}

impl fmt::Display for PublicationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Published => f.write_str("published"),
            Self::Unpublished => f.write_str("unpublished"),
            Self::Archived => f.write_str("archived"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaMetadata {
    pub title: String,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub description: Option<String>,
    pub credit: Option<String>,
    pub copyright: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub tags: BTreeSet<String>,
}

impl MediaMetadata {
    pub fn new(title: impl Into<String>) -> Result<Self, MediaModelError> {
        Ok(Self {
            title: require_non_empty("media_title", title.into())?,
            alt_text: None,
            caption: None,
            description: None,
            credit: None,
            copyright: None,
            width: None,
            height: None,
            tags: BTreeSet::new(),
        })
    }

    pub fn with_alt_text(mut self, alt_text: impl Into<String>) -> Result<Self, MediaModelError> {
        self.alt_text = Some(require_non_empty("media_alt_text", alt_text.into())?);
        Ok(self)
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Result<Self, MediaModelError> {
        self.caption = Some(require_non_empty("media_caption", caption.into())?);
        Ok(self)
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, MediaModelError> {
        self.description = Some(require_non_empty("media_description", description.into())?);
        Ok(self)
    }

    pub fn with_credit(mut self, credit: impl Into<String>) -> Result<Self, MediaModelError> {
        self.credit = Some(require_non_empty("media_credit", credit.into())?);
        Ok(self)
    }

    pub fn with_copyright(
        mut self,
        copyright: impl Into<String>,
    ) -> Result<Self, MediaModelError> {
        self.copyright = Some(require_non_empty("media_copyright", copyright.into())?);
        Ok(self)
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Result<Self, MediaModelError> {
        self.tags.insert(validate_token("media_tag", tag.into())?);
        Ok(self)
    }

    pub fn image_dimensions(&self) -> Option<(u32, u32)> {
        self.width.zip(self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTechnicalMetadata {
    pub content_type: String,
    pub byte_length: u64,
    pub fingerprint: ContentFingerprint,
}

impl MediaTechnicalMetadata {
    pub fn new(
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            content_type: require_non_empty("content_type", content_type.into())?,
            byte_length,
            fingerprint,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDerivativeKind {
    Thumbnail,
    Preview,
    Responsive,
    Archive,
    Custom,
}

impl fmt::Display for MediaDerivativeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thumbnail => f.write_str("thumbnail"),
            Self::Preview => f.write_str("preview"),
            Self::Responsive => f.write_str("responsive"),
            Self::Archive => f.write_str("archive"),
            Self::Custom => f.write_str("custom"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDerivative {
    pub id: MediaDerivativeId,
    pub kind: MediaDerivativeKind,
    pub title: String,
    pub mime_type: String,
    pub storage_policy: StoragePolicy,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl MediaDerivative {
    pub fn new(
        id: MediaDerivativeId,
        kind: MediaDerivativeKind,
        title: impl Into<String>,
        mime_type: impl Into<String>,
        storage_policy: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        storage_policy
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;

        Ok(Self {
            id,
            kind,
            title: require_non_empty("derivative_title", title.into())?,
            mime_type: require_non_empty("derivative_mime_type", mime_type.into())?,
            storage_policy,
            width: None,
            height: None,
        })
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAssetRevision {
    pub id: MediaRevisionId,
    pub asset_id: AssetId,
    pub logical_path: String,
    pub storage_policy: StoragePolicy,
    pub technical: MediaTechnicalMetadata,
    pub metadata: MediaMetadata,
    pub derivatives: Vec<MediaDerivative>,
}

impl MediaAssetRevision {
    pub fn new(
        id: MediaRevisionId,
        asset_id: AssetId,
        logical_path: impl Into<String>,
        storage_policy: StoragePolicy,
        technical: MediaTechnicalMetadata,
        metadata: MediaMetadata,
    ) -> Result<Self, MediaModelError> {
        storage_policy
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;

        Ok(Self {
            id,
            asset_id,
            logical_path: require_non_empty("logical_path", logical_path.into())?,
            storage_policy,
            technical,
            metadata,
            derivatives: Vec::new(),
        })
    }

    pub fn with_derivative(mut self, derivative: MediaDerivative) -> Self {
        self.derivatives.push(derivative);
        self
    }

    pub fn is_publicly_deliverable(&self) -> bool {
        self.storage_policy.delivery_mode == DeliveryMode::PublicCdn
            && self.storage_policy.sensitivity == Sensitivity::Public
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLibrary {
    pub id: MediaLibraryId,
    pub title: String,
    pub default_storage_policy: StoragePolicy,
    folders: BTreeMap<MediaFolderId, MediaFolder>,
    assets: BTreeMap<MediaAssetId, MediaAsset>,
}

impl MediaLibrary {
    pub fn new(
        id: MediaLibraryId,
        title: impl Into<String>,
        default_storage_policy: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        default_storage_policy
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;

        Ok(Self {
            id,
            title: require_non_empty("media_library_title", title.into())?,
            default_storage_policy,
            folders: BTreeMap::new(),
            assets: BTreeMap::new(),
        })
    }

    pub fn auth_entity(&self) -> Entity {
        Entity::media_library(self.id.to_string())
    }

    pub fn insert_folder(&mut self, folder: MediaFolder) -> Result<(), MediaModelError> {
        if folder.library_id != self.id {
            return Err(MediaModelError::MissingLibrary {
                library_id: self.id.to_string(),
            });
        }

        if self.folders.contains_key(&folder.id) {
            return Err(MediaModelError::DuplicateIdentifier {
                kind: "media folder",
                id: folder.id.to_string(),
            });
        }

        self.folders.insert(folder.id.clone(), folder);
        Ok(())
    }

    pub fn insert_asset(&mut self, asset: MediaAsset) -> Result<(), MediaModelError> {
        if asset.library_id != self.id {
            return Err(MediaModelError::MissingLibrary {
                library_id: self.id.to_string(),
            });
        }

        if self.assets.contains_key(&asset.id) {
            return Err(MediaModelError::DuplicateIdentifier {
                kind: "media asset",
                id: asset.id.to_string(),
            });
        }

        self.assets.insert(asset.id.clone(), asset);
        Ok(())
    }

    pub fn folder(&self, id: &MediaFolderId) -> Result<&MediaFolder, MediaModelError> {
        self.folders.get(id).ok_or_else(|| MediaModelError::MissingFolder {
            folder_id: id.to_string(),
        })
    }

    pub fn asset(&self, id: &MediaAssetId) -> Result<&MediaAsset, MediaModelError> {
        self.assets.get(id).ok_or_else(|| MediaModelError::MissingAsset {
            asset_id: id.to_string(),
        })
    }

    pub fn folder_path(&self, folder_id: &MediaFolderId) -> Result<String, MediaModelError> {
        let mut current = Some(folder_id.clone());
        let mut segments = Vec::new();

        while let Some(id) = current {
            let folder = self.folder(&id)?;
            segments.push(folder.slug.as_str().to_string());
            current = folder.parent_folder_id.clone();
        }

        segments.reverse();
        Ok(segments.join("/"))
    }

    pub fn asset_logical_path(&self, asset_id: &MediaAssetId) -> Result<String, MediaModelError> {
        let asset = self.asset(asset_id)?;
        let mut segments = vec![self.id.as_str().to_string()];

        if let Some(folder_id) = &asset.folder_id {
            segments.push(self.folder_path(folder_id)?);
        }

        segments.push(asset.slug.as_str().to_string());
        segments.push(asset.current_revision.id.as_str().to_string());
        Ok(segments.join("/"))
    }

    pub fn effective_storage_policy_for_folder(
        &self,
        folder_id: &MediaFolderId,
    ) -> Result<StoragePolicy, MediaModelError> {
        let folder = self.folder(folder_id)?;
        folder.resolved_storage_policy(self.default_storage_policy)
    }

    pub fn effective_storage_policy_for_asset(
        &self,
        asset_id: &MediaAssetId,
    ) -> Result<StoragePolicy, MediaModelError> {
        let asset = self.asset(asset_id)?;
        asset.resolved_storage_policy(self)
    }

    pub fn folders(&self) -> impl Iterator<Item = &MediaFolder> {
        self.folders.values()
    }

    pub fn assets(&self) -> impl Iterator<Item = &MediaAsset> {
        self.assets.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFolder {
    pub id: MediaFolderId,
    pub library_id: MediaLibraryId,
    pub parent_folder_id: Option<MediaFolderId>,
    pub slug: MediaSlug,
    pub title: String,
    pub storage_override: Option<StoragePolicyOverride>,
}

impl MediaFolder {
    pub fn new(
        id: MediaFolderId,
        library_id: MediaLibraryId,
        title: impl Into<String>,
        slug: MediaSlug,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            id,
            library_id,
            parent_folder_id: None,
            slug,
            title: require_non_empty("folder_title", title.into())?,
            storage_override: None,
        })
    }

    pub fn with_parent(mut self, parent_folder_id: MediaFolderId) -> Self {
        self.parent_folder_id = Some(parent_folder_id);
        self
    }

    pub fn with_storage_override(mut self, override_policy: StoragePolicyOverride) -> Self {
        self.storage_override = Some(override_policy);
        self
    }

    pub fn auth_entity(&self) -> Entity {
        Entity::asset_folder(self.id.to_string())
    }

    pub fn resolved_storage_policy(
        &self,
        base: StoragePolicy,
    ) -> Result<StoragePolicy, MediaModelError> {
        let policy = self
            .storage_override
            .as_ref()
            .map(|override_policy| override_policy.apply_to(base))
            .unwrap_or(base);
        policy
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;
        Ok(policy)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationState {
    pub status: PublicationStatus,
    pub live_revision: Option<MediaAssetRevision>,
}

impl PublicationState {
    pub fn is_live(&self) -> bool {
        self.live_revision.is_some()
    }

    pub fn live_revision(&self) -> Option<&MediaAssetRevision> {
        self.live_revision.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAsset {
    pub id: MediaAssetId,
    pub library_id: MediaLibraryId,
    pub folder_id: Option<MediaFolderId>,
    pub slug: MediaSlug,
    pub title: String,
    pub storage_override: Option<StoragePolicyOverride>,
    pub current_revision: MediaAssetRevision,
    pub publication: PublicationState,
    pub staged_replacement: Option<MediaAssetRevision>,
}

impl MediaAsset {
    pub fn new(
        id: MediaAssetId,
        library_id: MediaLibraryId,
        title: impl Into<String>,
        slug: MediaSlug,
        current_revision: MediaAssetRevision,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            id,
            library_id,
            folder_id: None,
            slug,
            title: require_non_empty("asset_title", title.into())?,
            storage_override: None,
            current_revision,
            publication: PublicationState {
                status: PublicationStatus::Draft,
                live_revision: None,
            },
            staged_replacement: None,
        })
    }

    pub fn with_folder(mut self, folder_id: MediaFolderId) -> Self {
        self.folder_id = Some(folder_id);
        self
    }

    pub fn with_storage_override(mut self, override_policy: StoragePolicyOverride) -> Self {
        self.storage_override = Some(override_policy);
        self
    }

    pub fn auth_entity(&self) -> Entity {
        Entity::media(self.id.to_string())
    }

    pub fn asset_entity(&self) -> Entity {
        Entity::asset(self.current_revision.asset_id.to_string())
    }

    pub fn publication(&self) -> &PublicationState {
        &self.publication
    }

    pub fn live_revision(&self) -> Result<&MediaAssetRevision, MediaModelError> {
        self.publication
            .live_revision()
            .ok_or_else(|| MediaModelError::MissingLiveRevision {
                asset_id: self.id.to_string(),
            })
    }

    pub fn has_pending_changes(&self) -> bool {
        (match self.publication.live_revision() {
            Some(live) => live.id != self.current_revision.id,
            None => true,
        }) || self.staged_replacement.is_some()
    }

    pub fn publish_current(&mut self) {
        self.publication.live_revision = Some(self.current_revision.clone());
        self.publication.status = PublicationStatus::Published;
    }

    pub fn unpublish(&mut self) -> Result<(), MediaModelError> {
        self.live_revision()?;
        self.publication.live_revision = None;
        self.publication.status = PublicationStatus::Unpublished;
        Ok(())
    }

    pub fn stage_replacement(&mut self, revision: MediaAssetRevision) {
        self.staged_replacement = Some(revision);
    }

    pub fn apply_staged_replacement(&mut self) -> Result<(), MediaModelError> {
        let revision = self
            .staged_replacement
            .take()
            .ok_or_else(|| MediaModelError::MissingStagedReplacement {
                asset_id: self.id.to_string(),
            })?;
        self.current_revision = revision;
        Ok(())
    }

    pub fn replace_current_revision(&mut self, revision: MediaAssetRevision) {
        self.current_revision = revision;
        self.staged_replacement = None;
    }

    pub fn resolved_storage_policy(
        &self,
        library: &MediaLibrary,
    ) -> Result<StoragePolicy, MediaModelError> {
        let mut policy = library.default_storage_policy;

        if let Some(folder_id) = &self.folder_id {
            policy = library.folder(folder_id)?.resolved_storage_policy(policy)?;
        }

        if let Some(override_policy) = &self.storage_override {
            policy = override_policy.apply_to(policy);
        }

        policy
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;
        Ok(policy)
    }

    pub fn auth_updates(&self) -> Vec<DefaultTupleUpdate> {
        let mut updates = vec![DefaultTupleUpdate::Write(DefaultTuple::new(
            self.auth_entity(),
            Relation::Library,
            DefaultSubject::entity(Entity::media_library(self.library_id.to_string())),
        ))];

        if let Some(folder_id) = &self.folder_id {
            updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                self.asset_entity(),
                Relation::Folder,
                DefaultSubject::entity(Entity::asset_folder(folder_id.to_string())),
            )));
        }

        if self.publication.status == PublicationStatus::Published
            && self.current_revision.is_publicly_deliverable()
        {
            updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                self.asset_entity(),
                Relation::ReadPublic,
                DefaultSubject::entity(Entity::any_user()),
            )));
        } else {
            updates.push(DefaultTupleUpdate::Delete(DefaultTuple::new(
                self.asset_entity(),
                Relation::ReadPublic,
                DefaultSubject::entity(Entity::any_user()),
            )));
        }

        updates
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAccessGrant {
    pub capability: Capability,
    pub entity: Entity,
}

impl MediaAccessGrant {
    pub fn new(capability: Capability, entity: Entity) -> Self {
        Self { capability, entity }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaAccessKind {
    Read,
    ReadPublic,
    Publish,
    Replace,
    ManageStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLibraryPolicy {
    pub folder_default: StoragePolicy,
    pub asset_default: StoragePolicy,
}

impl MediaLibraryPolicy {
    pub fn new(folder_default: StoragePolicy, asset_default: StoragePolicy) -> Result<Self, MediaModelError> {
        folder_default
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;
        asset_default
            .validate()
            .map_err(|error| MediaModelError::StoragePolicy { error })?;

        Ok(Self {
            folder_default,
            asset_default,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaDerivativeHint {
    PreserveOriginal,
    CreateThumbnail,
    CreatePreview,
    CreateResponsiveSet,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLibraryOverview {
    pub library_id: MediaLibraryId,
    pub folder_count: usize,
    pub asset_count: usize,
    pub published_asset_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaModule {
    name: String,
    config_namespace: String,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            name: "media".to_string(),
            config_namespace: "media".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }
}

impl Default for MediaModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for MediaModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::AssetRead,
                Capability::AssetPublish,
                Capability::AssetReplace,
                Capability::AssetManageStorage,
            ])
            .with_optional_capabilities(vec![
                Capability::AssetReadPublic,
                Capability::AdminShellAccess,
                Capability::AdminAuditRead,
                Capability::CmsPageRead,
                Capability::SeoMetadataEdit,
                Capability::I18nTranslationEdit,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::required(
                    Capability::AssetPublish,
                    ["asset", "media"],
                ),
                CapabilityContract::required(
                    Capability::AssetReplace,
                    ["asset", "media"],
                ),
                CapabilityContract::required(
                    Capability::AssetManageStorage,
                    ["asset", "asset_folder", "media_library"],
                ),
                CapabilityContract::optional(
                    Capability::AssetReadPublic,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(
                    Capability::AdminAuditRead,
                    ["audit_entry"],
                ),
                CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
                CapabilityContract::optional(
                    Capability::SeoMetadataEdit,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["asset", "media"],
                ),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::optional(
                    "admin",
                    "Media contributes library and storage-policy workflows to the shared admin shell when installed",
                ),
                ModuleDependency::optional(
                    "cms",
                    "Managed assets can be referenced from CMS pages and editorial workflows",
                ),
                ModuleDependency::optional(
                    "events",
                    "Managed assets can supply event media and downloadable resources",
                ),
                ModuleDependency::optional(
                    "commerce",
                    "Managed assets can supply product media and downloadable order assets",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Storage,
                CoreServiceDependency::Assets,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Seo,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "media.libraries",
                    10,
                    "Creates media-library, folder, and inherited storage-policy tables",
                ),
                MigrationContract::new(
                    "media.assets",
                    20,
                    "Creates managed asset, revision, and publication workflow tables",
                ),
                MigrationContract::new(
                    "media.derivatives",
                    30,
                    "Creates derivative generation, sync backlog, and technical metadata tables",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new(
                    "media.library",
                    RouteSurfaceKind::AdminPage,
                    "/admin/media",
                )
                .gated_by(Capability::AssetRead),
                RouteSurface::new(
                    "media.delivery",
                    RouteSurfaceKind::Asset,
                    "/media/files/{asset_id}",
                )
                .gated_by(Capability::AssetRead),
                RouteSurface::new(
                    "media.storage",
                    RouteSurfaceKind::AdminPage,
                    "/admin/media/storage",
                )
                .gated_by(Capability::AssetManageStorage),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "media.derivatives.generate",
                    JobTriggerKind::InlineFollowup,
                    true,
                    "Generates thumbnails, previews, and responsive derivatives after asset ingest",
                ),
                JobContract::new(
                    "media.storage.sync",
                    JobTriggerKind::Scheduled,
                    true,
                    "Reconciles write-through object storage and tracks exceptional local-only assets",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "media.asset.published",
                    Some("media.storage.sync"),
                    "Ensures newly published assets are durably available across object storage and delivery surfaces",
                ),
                EventSubscription::new(
                    "media.asset.replaced",
                    Some("media.derivatives.generate"),
                    "Regenerates derived media artifacts when a staged replacement goes live",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::StoragePolicy,
                    "media.storage-policy",
                    "Bridges folder defaults, per-upload overrides, and delivery modes onto the shared storage engine",
                ),
                IntegrationPoint::new(
                    IntegrationKind::AuthPublication,
                    "media.publication",
                    "Treats managed-asset publication as an auth-governed state transition instead of a file-copy side effect",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SeoMetadata,
                    "media.metadata",
                    "Supplies alt text, dimensions, and canonical metadata for downstream consumers",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::StoragePolicyAware,
                ModuleBehavior::AuthGovernedPublication,
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AccessibleAdminUi,
            ])
            .with_extension_slots(vec![
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::AdminWidget,
                    "media.asset.sidebar",
                    "Allows bounded customer widgets to augment media detail views and review workflows",
                ),
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::RenderHook,
                    "media.asset.metadata",
                    "Allows controlled metadata enrichment without bypassing the shared storage and publication rules",
                ),
            ])
            .with_admin_resources(vec![
                AdminResourceContribution::new(
                    "media.library",
                    "/admin/media",
                    "Media library",
                    "Media",
                    AdminNavigationSection::Media,
                    AdminContributionKind::ResourceIndex,
                    Capability::AssetRead,
                ),
                AdminResourceContribution::new(
                    "media.storage",
                    "/admin/media/storage",
                    "Storage policies",
                    "Storage",
                    AdminNavigationSection::Media,
                    AdminContributionKind::Settings,
                    Capability::AssetManageStorage,
                ),
            ])
            .with_http_surfaces(vec![
                HttpSurfaceContribution::page(
                    "media.library",
                    HttpSurfaceArea::Admin,
                    "/admin/media",
                    "media/library",
                )
                .gated_by(Capability::AssetRead),
                HttpSurfaceContribution::file(
                    "media.delivery",
                    HttpSurfaceArea::Public,
                    "/media/files/{asset_id}",
                    "media/files/{asset_id}",
                    "application/octet-stream",
                    HttpFileDeliveryMode::AppProxy,
                )
                .gated_by(Capability::AssetRead),
                HttpSurfaceContribution::page(
                    "media.storage",
                    HttpSurfaceArea::Admin,
                    "/admin/media/storage",
                    "media/storage",
                )
                .gated_by(Capability::AssetManageStorage),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.media.libraries",
            "Media libraries, folder trees, and library-level policy defaults",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.folders",
            "Managed media folders and storage policy overrides",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.assets",
            "Managed media assets, revisions, publication state, and reuse across modules",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.metadata",
            "Metadata capture, derived metadata, and image handling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.replacement",
            "Replacement workflows and revision promotion for managed assets",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.storage",
            "Storage policy interplay, delivery modes, and local-only overrides",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.admin",
            "Media admin resources and operator workflows",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_libraries").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create media-library and folder policy storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_libraries (id TEXT PRIMARY KEY, name TEXT NOT NULL, default_policy TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_assets").expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create managed media asset and revision storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_assets (id TEXT PRIMARY KEY, library_id TEXT NOT NULL, slug TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_derivatives").expect("constant migration id is valid"),
                owner,
                30,
                "Create media derivative and sync backlog storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_derivatives (id TEXT PRIMARY KEY, asset_id TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        Some(plan)
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MediaModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(MediaModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(MediaModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(value: &str) -> ContentFingerprint {
        ContentFingerprint::new(davenda_assets::FingerprintAlgorithm::Sha256, value).unwrap()
    }

    fn storage_public() -> StoragePolicy {
        StoragePolicy::public_asset()
    }

    fn storage_private() -> StoragePolicy {
        StoragePolicy::private_shared()
    }

    #[test]
    fn metadata_and_revision_capture_the_core_media_fields() {
        let metadata = MediaMetadata::new("Spring Campaign")
            .unwrap()
            .with_alt_text("A bright hero image")
            .unwrap()
            .with_caption("Spring campaign hero")
            .unwrap()
            .with_description("Hero image used for the spring launch")
            .unwrap()
            .with_credit("OpenAI Studio")
            .unwrap()
            .with_dimensions(1200, 800)
            .with_tag("hero")
            .unwrap()
            .with_tag("campaign")
            .unwrap();

        let technical = MediaTechnicalMetadata::new(
            "image/jpeg",
            42_000,
            fingerprint("abc123"),
        )
        .unwrap();
        let revision = MediaAssetRevision::new(
            MediaRevisionId::new("rev-1").unwrap(),
            AssetId::new("asset-1").unwrap(),
            "media/spring/rev-1.jpg",
            storage_public(),
            technical,
            metadata,
        )
        .unwrap()
        .with_derivative(
            MediaDerivative::new(
                MediaDerivativeId::new("thumb").unwrap(),
                MediaDerivativeKind::Thumbnail,
                "Thumbnail",
                "image/webp",
                storage_public(),
            )
            .unwrap()
            .with_dimensions(400, 267),
        );

        assert_eq!(revision.metadata.title, "Spring Campaign");
        assert_eq!(revision.metadata.image_dimensions(), Some((1200, 800)));
        assert_eq!(revision.derivatives.len(), 1);
        assert!(revision.is_publicly_deliverable());
    }

    #[test]
    fn folder_and_asset_policies_compose_in_order() {
        let library = MediaLibrary::new(
            MediaLibraryId::new("library").unwrap(),
            "Site media",
            storage_private(),
        )
        .unwrap();
        let folder = MediaFolder::new(
            MediaFolderId::new("folder").unwrap(),
            library.id.clone(),
            "Campaigns",
            MediaSlug::new("campaigns").unwrap(),
        )
        .unwrap()
        .with_storage_override(StoragePolicyOverride {
            delivery_mode: Some(DeliveryMode::SignedUrl),
            sync_mode: Some(davenda_storage::SyncMode::ObjectStore),
            sensitivity: Some(Sensitivity::Restricted),
        });

        let technical = MediaTechnicalMetadata::new(
            "image/png",
            1_024,
            fingerprint("def456"),
        )
        .unwrap();
        let revision = MediaAssetRevision::new(
            MediaRevisionId::new("rev-2").unwrap(),
            AssetId::new("asset-2").unwrap(),
            "media/campaigns/rev-2.png",
            storage_private(),
            technical,
            MediaMetadata::new("Campaign image").unwrap(),
        )
        .unwrap();
        let asset = MediaAsset::new(
            MediaAssetId::new("asset-media-1").unwrap(),
            library.id.clone(),
            "Campaign image",
            MediaSlug::new("campaign-image").unwrap(),
            revision,
        )
        .unwrap()
        .with_folder(folder.id.clone())
        .with_storage_override(StoragePolicyOverride::force_local_only());

        let mut library = library;
        library.insert_folder(folder).unwrap();
        library.insert_asset(asset).unwrap();

        let policy = library
            .effective_storage_policy_for_asset(&MediaAssetId::new("asset-media-1").unwrap())
            .unwrap();

        assert_eq!(policy.delivery_mode, DeliveryMode::LocalOnly);
        assert_eq!(policy.sync_mode, davenda_storage::SyncMode::LocalOnly);
        assert_eq!(policy.sensitivity, Sensitivity::Secret);
    }

    #[test]
    fn publication_state_emits_public_read_auth_tuples_for_public_assets() {
        let library = MediaLibrary::new(
            MediaLibraryId::new("library").unwrap(),
            "Site media",
            storage_public(),
        )
        .unwrap();
        let technical = MediaTechnicalMetadata::new(
            "image/jpeg",
            10_000,
            fingerprint("ghi789"),
        )
        .unwrap();
        let revision = MediaAssetRevision::new(
            MediaRevisionId::new("rev-3").unwrap(),
            AssetId::new("asset-3").unwrap(),
            "media/public/rev-3.jpg",
            storage_public(),
            technical,
            MediaMetadata::new("Public image").unwrap(),
        )
        .unwrap();
        let mut asset = MediaAsset::new(
            MediaAssetId::new("asset-media-2").unwrap(),
            library.id.clone(),
            "Public image",
            MediaSlug::new("public-image").unwrap(),
            revision,
        )
        .unwrap()
        .with_folder(MediaFolderId::new("folder-2").unwrap());
        asset.publish_current();

        let updates = asset.auth_updates();
        let expected = DefaultTupleUpdate::Write(DefaultTuple::new(
            Entity::asset("asset-3"),
            Relation::ReadPublic,
            DefaultSubject::entity(Entity::any_user()),
        ));

        assert!(updates.contains(&expected));
    }

    #[test]
    fn replacement_workflow_tracks_staged_revisions() {
        let technical = MediaTechnicalMetadata::new(
            "image/jpeg",
            10_000,
            fingerprint("jkl012"),
        )
        .unwrap();
        let current = MediaAssetRevision::new(
            MediaRevisionId::new("rev-4").unwrap(),
            AssetId::new("asset-4").unwrap(),
            "media/current.jpg",
            storage_private(),
            technical.clone(),
            MediaMetadata::new("Current image").unwrap(),
        )
        .unwrap();
        let staged = MediaAssetRevision::new(
            MediaRevisionId::new("rev-5").unwrap(),
            AssetId::new("asset-5").unwrap(),
            "media/staged.jpg",
            storage_private(),
            technical,
            MediaMetadata::new("Replacement image").unwrap(),
        )
        .unwrap();
        let mut asset = MediaAsset::new(
            MediaAssetId::new("asset-media-3").unwrap(),
            MediaLibraryId::new("library").unwrap(),
            "Current image",
            MediaSlug::new("current-image").unwrap(),
            current,
        )
        .unwrap();

        asset.stage_replacement(staged);
        assert!(asset.staged_replacement.is_some());
        asset.apply_staged_replacement().unwrap();
        assert_eq!(asset.current_revision.metadata.title, "Replacement image");
        assert!(asset.staged_replacement.is_none());
    }

    #[test]
    fn module_manifest_and_registration_match_first_party_patterns() {
        let module = MediaModule::default();
        let manifest = module.manifest();
        assert_eq!(manifest.name, "media");
        assert!(manifest
            .required_capabilities
            .contains(&Capability::AssetManageStorage));
        assert!(manifest
            .optional_capabilities
            .contains(&Capability::AdminShellAccess));
        assert_eq!(manifest.migrations.len(), 3);
        assert_eq!(manifest.route_surfaces.len(), 3);
        assert_eq!(manifest.http_surfaces.len(), 3);
        assert_eq!(manifest.jobs.len(), 2);
        assert_eq!(manifest.event_subscriptions.len(), 2);
        assert_eq!(manifest.admin_resources.len(), 2);
        assert!(manifest
            .behaviors
            .contains(&ModuleBehavior::AuthGovernedPublication));
        assert!(manifest
            .extension_slots
            .iter()
            .any(|slot| slot.kind == ExtensionSlotKind::AdminWidget));
        assert_eq!(
            module
                .install_migration_plan()
                .expect("media migration plan")
                .ordered_steps()
                .len(),
            3
        );

        let mut registry = ServiceRegistry::new();
        module.register(&mut registry).unwrap();
        let service_ids = registry.services().map(|service| service.id.clone()).collect::<Vec<_>>();
        assert!(service_ids.contains(&"module.media.assets".to_string()));
        assert!(service_ids.contains(&"module.media.storage".to_string()));
    }
}

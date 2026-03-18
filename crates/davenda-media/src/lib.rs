use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use davenda_auth::Capability;
use davenda_core::{ModuleManifest, PlatformModule, RegistrationError, ServiceRegistry};
use davenda_storage::{DeliveryMode, Sensitivity, StoragePolicy, StoragePolicyOverride};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidPath {
        field: &'static str,
        value: String,
    },
    DuplicateFolder {
        folder_id: String,
    },
    DuplicateAsset {
        asset_id: String,
    },
    MissingFolder {
        folder_id: String,
    },
    MissingAsset {
        asset_id: String,
    },
    PublicDeliveryRequiresPublication {
        asset_id: String,
    },
    InvalidReplacement {
        current_revision: u32,
        next_revision: u32,
    },
}

impl fmt::Display for MediaModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidPath { field, value } => {
                write!(
                    f,
                    "`{field}` must be a normalized relative path, got `{value}`"
                )
            }
            Self::DuplicateFolder { folder_id } => write!(f, "folder `{folder_id}` is duplicated"),
            Self::DuplicateAsset { asset_id } => write!(f, "asset `{asset_id}` is duplicated"),
            Self::MissingFolder { folder_id } => write!(f, "folder `{folder_id}` was not found"),
            Self::MissingAsset { asset_id } => write!(f, "asset `{asset_id}` was not found"),
            Self::PublicDeliveryRequiresPublication { asset_id } => write!(
                f,
                "asset `{asset_id}` cannot use public delivery until it is published"
            ),
            Self::InvalidReplacement {
                current_revision,
                next_revision,
            } => write!(
                f,
                "replacement revision must increment by one, got current={current_revision} next={next_revision}"
            ),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MimeType(String);

impl MimeType {
    pub fn new(value: impl Into<String>) -> Result<Self, MediaModelError> {
        Ok(Self(validate_mime_type(value.into())?))
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Document,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPublicationState {
    Draft,
    Published,
    Unpublished,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaMetadata {
    pub title: String,
    pub alt_text: Option<String>,
    pub byte_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub checksum: String,
}

impl MediaMetadata {
    pub fn new(
        title: impl Into<String>,
        alt_text: Option<String>,
        byte_size: u64,
        width: Option<u32>,
        height: Option<u32>,
        checksum: impl Into<String>,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            title: require_non_empty("media_title", title.into())?,
            alt_text: alt_text.map(|value| value.trim().to_string()),
            byte_size,
            width,
            height,
            checksum: require_non_empty("checksum", checksum.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFolder {
    pub id: MediaFolderId,
    pub name: String,
    pub path_prefix: String,
    pub default_policy: StoragePolicy,
}

impl MediaFolder {
    pub fn new(
        id: MediaFolderId,
        name: impl Into<String>,
        path_prefix: impl Into<String>,
        default_policy: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            id,
            name: require_non_empty("folder_name", name.into())?,
            path_prefix: validate_path("path_prefix", path_prefix.into())?,
            default_policy,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedMediaAsset {
    pub id: MediaAssetId,
    pub folder_id: MediaFolderId,
    pub kind: MediaKind,
    pub file_name: String,
    pub path: String,
    pub mime_type: MimeType,
    pub metadata: MediaMetadata,
    pub storage_policy: StoragePolicy,
    pub publication_state: MediaPublicationState,
    pub revision: u32,
}

impl ManagedMediaAsset {
    pub fn new(
        id: MediaAssetId,
        folder_id: MediaFolderId,
        kind: MediaKind,
        file_name: impl Into<String>,
        path: impl Into<String>,
        mime_type: MimeType,
        metadata: MediaMetadata,
        storage_policy: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            id,
            folder_id,
            kind,
            file_name: require_non_empty("file_name", file_name.into())?,
            path: validate_path("asset_path", path.into())?,
            mime_type,
            metadata,
            storage_policy,
            publication_state: MediaPublicationState::Draft,
            revision: 1,
        })
    }

    pub fn publish(&mut self) -> Result<(), MediaModelError> {
        if self.storage_policy.delivery_mode == DeliveryMode::PublicCdn
            && self.storage_policy.sensitivity != Sensitivity::Public
        {
            return Err(MediaModelError::PublicDeliveryRequiresPublication {
                asset_id: self.id.to_string(),
            });
        }

        self.publication_state = MediaPublicationState::Published;
        Ok(())
    }

    pub fn unpublish(&mut self) {
        self.publication_state = MediaPublicationState::Unpublished;
    }

    pub fn replace(
        &mut self,
        next_revision: u32,
        metadata: MediaMetadata,
        storage_override: Option<StoragePolicyOverride>,
    ) -> Result<(), MediaModelError> {
        if next_revision != self.revision + 1 {
            return Err(MediaModelError::InvalidReplacement {
                current_revision: self.revision,
                next_revision,
            });
        }

        self.revision = next_revision;
        self.metadata = metadata;
        if let Some(storage_override) = storage_override {
            self.storage_policy = storage_override.apply_to(self.storage_policy);
        }
        self.publication_state = MediaPublicationState::Draft;
        Ok(())
    }

    pub fn can_be_delivered_publicly(&self) -> bool {
        self.publication_state == MediaPublicationState::Published
            && self.storage_policy.delivery_mode == DeliveryMode::PublicCdn
            && self.storage_policy.sensitivity == Sensitivity::Public
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLibrary {
    pub id: MediaLibraryId,
    pub title: String,
    folders: BTreeMap<MediaFolderId, MediaFolder>,
    assets: BTreeMap<MediaAssetId, ManagedMediaAsset>,
}

impl MediaLibrary {
    pub fn new(id: MediaLibraryId, title: impl Into<String>) -> Result<Self, MediaModelError> {
        Ok(Self {
            id,
            title: require_non_empty("library_title", title.into())?,
            folders: BTreeMap::new(),
            assets: BTreeMap::new(),
        })
    }

    pub fn add_folder(&mut self, folder: MediaFolder) -> Result<(), MediaModelError> {
        if self.folders.contains_key(&folder.id) {
            return Err(MediaModelError::DuplicateFolder {
                folder_id: folder.id.to_string(),
            });
        }

        self.folders.insert(folder.id.clone(), folder);
        Ok(())
    }

    pub fn ingest_asset(
        &mut self,
        asset_id: MediaAssetId,
        folder_id: &MediaFolderId,
        kind: MediaKind,
        file_name: impl Into<String>,
        relative_path: impl Into<String>,
        mime_type: MimeType,
        metadata: MediaMetadata,
        policy_override: Option<StoragePolicyOverride>,
    ) -> Result<&ManagedMediaAsset, MediaModelError> {
        if self.assets.contains_key(&asset_id) {
            return Err(MediaModelError::DuplicateAsset {
                asset_id: asset_id.to_string(),
            });
        }

        let folder = self
            .folders
            .get(folder_id)
            .ok_or_else(|| MediaModelError::MissingFolder {
                folder_id: folder_id.to_string(),
            })?;
        let policy = policy_override
            .map(|override_policy| override_policy.apply_to(folder.default_policy))
            .unwrap_or(folder.default_policy);
        let asset = ManagedMediaAsset::new(
            asset_id.clone(),
            folder_id.clone(),
            kind,
            file_name,
            format!("{}/{}", folder.path_prefix, relative_path.into()),
            mime_type,
            metadata,
            policy,
        )?;
        self.assets.insert(asset_id.clone(), asset);
        Ok(self.assets.get(&asset_id).expect("inserted asset exists"))
    }

    pub fn asset(&self, asset_id: &MediaAssetId) -> Option<&ManagedMediaAsset> {
        self.assets.get(asset_id)
    }

    pub fn asset_mut(&mut self, asset_id: &MediaAssetId) -> Option<&mut ManagedMediaAsset> {
        self.assets.get_mut(asset_id)
    }

    pub fn public_assets(&self) -> Vec<&ManagedMediaAsset> {
        self.assets
            .values()
            .filter(|asset| asset.can_be_delivered_publicly())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResourceDescriptor {
    pub route: String,
    pub capability: Capability,
    pub title: String,
}

impl AdminResourceDescriptor {
    pub fn new(
        route: impl Into<String>,
        capability: Capability,
        title: impl Into<String>,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            route: validate_route("admin_route", route.into())?,
            capability,
            title: require_non_empty("admin_title", title.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceDescriptor>,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            name: "media".to_string(),
            config_namespace: "media".to_string(),
            admin_resources: vec![
                AdminResourceDescriptor::new(
                    "/admin/media/library",
                    Capability::AssetRead,
                    "Media library",
                )
                .expect("constant admin route is valid"),
                AdminResourceDescriptor::new(
                    "/admin/media/publishing",
                    Capability::AssetPublish,
                    "Publishing",
                )
                .expect("constant admin route is valid"),
                AdminResourceDescriptor::new(
                    "/admin/media/storage",
                    Capability::AssetManageStorage,
                    "Storage policy",
                )
                .expect("constant admin route is valid"),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceDescriptor] {
        &self.admin_resources
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
                Capability::AssetReadPublic,
                Capability::AssetPublish,
                Capability::AssetReplace,
                Capability::AssetManageStorage,
            ])
            .with_config_namespace(self.config_namespace.clone())
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.media.library",
            "Media libraries, folder organization, and metadata management",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.assets",
            "Managed media assets with publication workflow and replacement history",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.storage",
            "Storage policy binding and delivery-mode aware media handling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.admin",
            "Media admin resources for library, publishing, and storage operations",
        )
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MediaModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(MediaModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_mime_type(value: String) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MediaModelError::EmptyField { field: "mime_type" });
    }

    let mut parts = trimmed.split('/');
    let top = parts.next();
    let sub = parts.next();

    if parts.next().is_some()
        || top.is_none()
        || sub.is_none()
        || !top
            .unwrap()
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '+'))
        || !sub
            .unwrap()
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '+' | '.'))
    {
        Err(MediaModelError::InvalidToken {
            field: "mime_type",
            value: trimmed.to_string(),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_route(field: &'static str, value: String) -> Result<String, MediaModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(MediaModelError::InvalidPath {
            field,
            value: route,
        })
    }
}

fn validate_path(field: &'static str, value: String) -> Result<String, MediaModelError> {
    let path = require_non_empty(field, value)?;
    if path.starts_with('/')
        || path.contains("..")
        || path.split('/').any(|segment| segment.trim().is_empty())
    {
        Err(MediaModelError::InvalidPath { field, value: path })
    } else {
        Ok(path)
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

    fn image_metadata() -> MediaMetadata {
        MediaMetadata::new(
            "Hero image",
            Some("Spring tasting hero".to_string()),
            512_000,
            Some(1200),
            Some(800),
            "sha256:hero",
        )
        .unwrap()
    }

    #[test]
    fn media_library_applies_folder_policy_and_override_rules() {
        let mut library =
            MediaLibrary::new(MediaLibraryId::new("main").unwrap(), "Main library").unwrap();
        library
            .add_folder(
                MediaFolder::new(
                    MediaFolderId::new("public-images").unwrap(),
                    "Public images",
                    "images/public",
                    StoragePolicy::public_upload(),
                )
                .unwrap(),
            )
            .unwrap();

        let asset = library
            .ingest_asset(
                MediaAssetId::new("hero-image").unwrap(),
                &MediaFolderId::new("public-images").unwrap(),
                MediaKind::Image,
                "hero.jpg",
                "hero.jpg",
                MimeType::new("image/jpeg").unwrap(),
                image_metadata(),
                Some(StoragePolicyOverride::force_local_only()),
            )
            .unwrap();

        assert_eq!(asset.storage_policy.delivery_mode, DeliveryMode::LocalOnly);
        assert_eq!(asset.storage_policy.sensitivity, Sensitivity::Secret);
    }

    #[test]
    fn publishing_and_replacement_follow_media_workflow() {
        let mut asset = ManagedMediaAsset::new(
            MediaAssetId::new("hero-image").unwrap(),
            MediaFolderId::new("public-images").unwrap(),
            MediaKind::Image,
            "hero.jpg",
            "images/public/hero.jpg",
            MimeType::new("image/jpeg").unwrap(),
            image_metadata(),
            StoragePolicy::public_upload(),
        )
        .unwrap();

        asset.publish().unwrap();
        assert!(asset.can_be_delivered_publicly());

        asset
            .replace(
                2,
                MediaMetadata::new(
                    "Hero image v2",
                    Some("Updated hero".to_string()),
                    640_000,
                    Some(1600),
                    Some(900),
                    "sha256:hero-v2",
                )
                .unwrap(),
                None,
            )
            .unwrap();
        assert_eq!(asset.revision, 2);
        assert_eq!(asset.publication_state, MediaPublicationState::Draft);
    }

    #[test]
    fn library_filters_publicly_deliverable_assets() {
        let mut library =
            MediaLibrary::new(MediaLibraryId::new("main").unwrap(), "Main library").unwrap();
        library
            .add_folder(
                MediaFolder::new(
                    MediaFolderId::new("public-images").unwrap(),
                    "Public images",
                    "images/public",
                    StoragePolicy::public_upload(),
                )
                .unwrap(),
            )
            .unwrap();
        library
            .add_folder(
                MediaFolder::new(
                    MediaFolderId::new("restricted-docs").unwrap(),
                    "Restricted docs",
                    "docs/restricted",
                    StoragePolicy::private_shared(),
                )
                .unwrap(),
            )
            .unwrap();

        let public_id = MediaAssetId::new("hero-image").unwrap();
        library
            .ingest_asset(
                public_id.clone(),
                &MediaFolderId::new("public-images").unwrap(),
                MediaKind::Image,
                "hero.jpg",
                "hero.jpg",
                MimeType::new("image/jpeg").unwrap(),
                image_metadata(),
                None,
            )
            .unwrap();
        library.asset_mut(&public_id).unwrap().publish().unwrap();

        let private_id = MediaAssetId::new("member-pack").unwrap();
        library
            .ingest_asset(
                private_id.clone(),
                &MediaFolderId::new("restricted-docs").unwrap(),
                MediaKind::Document,
                "member-pack.pdf",
                "member-pack.pdf",
                MimeType::new("application/pdf").unwrap(),
                MediaMetadata::new(
                    "Member pack",
                    None,
                    128_000,
                    None,
                    None,
                    "sha256:member-pack",
                )
                .unwrap(),
                None,
            )
            .unwrap();
        library.asset_mut(&private_id).unwrap().publish().unwrap();

        let public_assets = library.public_assets();
        assert_eq!(public_assets.len(), 1);
        assert_eq!(public_assets[0].id, public_id);
    }

    #[test]
    fn media_module_manifest_and_admin_resources_match_contracts() {
        let module = MediaModule::new();
        let manifest = module.manifest();

        assert_eq!(manifest.name, "media");
        assert!(
            manifest
                .required_capabilities
                .contains(&Capability::AssetManageStorage)
        );
        assert_eq!(module.admin_resources().len(), 3);

        let mut registry = ServiceRegistry::new();
        module.register(&mut registry).unwrap();
        assert!(
            registry
                .services()
                .any(|service| service.id == "module.media.library")
        );
    }
}

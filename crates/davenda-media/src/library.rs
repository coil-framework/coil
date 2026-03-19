use crate::asset::MediaAsset;
use crate::error::MediaModelError;
use crate::identifiers::{MediaAssetId, MediaFolderId, MediaLibraryId, MediaSlug};
use crate::validation::require_non_empty;
use davenda_auth::Entity;
use davenda_storage::{StoragePolicy, StoragePolicyOverride};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAccessGrant {
    pub capability: davenda_auth::Capability,
    pub entity: Entity,
}

impl MediaAccessGrant {
    pub fn new(capability: davenda_auth::Capability, entity: Entity) -> Self {
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
    pub fn new(
        folder_default: StoragePolicy,
        asset_default: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        folder_default.validate().map_err(MediaModelError::from)?;
        asset_default.validate().map_err(MediaModelError::from)?;

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
        policy.validate().map_err(MediaModelError::from)?;
        Ok(policy)
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
            .map_err(MediaModelError::from)?;

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
        self.folders
            .get(id)
            .ok_or_else(|| MediaModelError::MissingFolder {
                folder_id: id.to_string(),
            })
    }

    pub fn asset(&self, id: &MediaAssetId) -> Result<&MediaAsset, MediaModelError> {
        self.assets
            .get(id)
            .ok_or_else(|| MediaModelError::MissingAsset {
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

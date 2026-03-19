use crate::error::MediaModelError;
use crate::identifiers::{MediaAssetId, MediaFolderId, MediaLibraryId, MediaRevisionId, MediaSlug};
use crate::library::MediaLibrary;
use crate::model::{
    MediaDerivative, MediaMetadata, MediaTechnicalMetadata, PublicationStatus, is_public_revision,
};
use crate::validation::require_non_empty;
use davenda_assets::AssetId;
use davenda_auth::{DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation};
use davenda_storage::{StoragePolicy, StoragePolicyOverride};

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
        storage_policy.validate().map_err(MediaModelError::from)?;

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
        is_public_revision(&self.storage_policy)
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
        let revision = self.staged_replacement.take().ok_or_else(|| {
            MediaModelError::MissingStagedReplacement {
                asset_id: self.id.to_string(),
            }
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

        policy.validate().map_err(MediaModelError::from)?;
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

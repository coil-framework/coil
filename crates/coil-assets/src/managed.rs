use super::*;
use coil_auth::{DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation};
use coil_storage::{
    SingleNodeEscapeHatchPlanner, StoragePlan, StoragePlanRequest, StoragePlanner,
    StoragePolicyOverride,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAssetRevision {
    id: RevisionId,
    storage_plan: StoragePlan,
    content_type: String,
    byte_length: u64,
    fingerprint: ContentFingerprint,
}

impl ManagedAssetRevision {
    pub fn plan(
        id: RevisionId,
        planner: &StoragePlanner,
        logical_path: impl Into<String>,
        override_policy: Option<StoragePolicyOverride>,
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<Self, AssetModelError> {
        let mut request = StoragePlanRequest::new(logical_path);
        if let Some(override_policy) = override_policy {
            request = request.with_override(override_policy);
        }

        let storage_plan = planner
            .plan_scalable_write(request)
            .map_err(AssetModelError::Storage)?;
        Self::new(id, storage_plan, content_type, byte_length, fingerprint)
    }

    pub fn plan_with_single_node_escape_hatch(
        id: RevisionId,
        planner: &SingleNodeEscapeHatchPlanner,
        logical_path: impl Into<String>,
        override_policy: Option<StoragePolicyOverride>,
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<Self, AssetModelError> {
        let mut request = StoragePlanRequest::new(logical_path);
        if let Some(override_policy) = override_policy {
            request = request.with_override(override_policy);
        }

        let storage_plan = planner
            .plan_write(request)
            .map_err(AssetModelError::Storage)?;
        Self::new(id, storage_plan, content_type, byte_length, fingerprint)
    }

    pub fn new(
        id: RevisionId,
        storage_plan: StoragePlan,
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<Self, AssetModelError> {
        Ok(Self {
            id,
            storage_plan,
            content_type: require_non_empty("content_type", content_type.into())?,
            byte_length,
            fingerprint,
        })
    }

    pub fn id(&self) -> &RevisionId {
        &self.id
    }

    pub fn storage_plan(&self) -> &StoragePlan {
        &self.storage_plan
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn fingerprint(&self) -> &ContentFingerprint {
        &self.fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStatus {
    Draft,
    Published,
    Unpublished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationTransition {
    PublishCurrent,
    Unpublish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationState {
    status: PublicationStatus,
    live_revision: Option<ManagedAssetRevision>,
}

impl PublicationState {
    pub fn status(&self) -> PublicationStatus {
        self.status
    }

    pub fn is_published(&self) -> bool {
        self.status == PublicationStatus::Published
    }

    pub fn live_revision(&self) -> Option<&ManagedAssetRevision> {
        self.live_revision.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAsset {
    id: AssetId,
    display_name: String,
    current_revision: ManagedAssetRevision,
    publication: PublicationState,
}

impl ManagedAsset {
    pub fn new(
        id: AssetId,
        display_name: impl Into<String>,
        initial_revision: ManagedAssetRevision,
    ) -> Result<Self, AssetModelError> {
        Ok(Self {
            id,
            display_name: require_non_empty("display_name", display_name.into())?,
            current_revision: initial_revision,
            publication: PublicationState {
                status: PublicationStatus::Draft,
                live_revision: None,
            },
        })
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn current_revision(&self) -> &ManagedAssetRevision {
        &self.current_revision
    }

    pub fn publication(&self) -> &PublicationState {
        &self.publication
    }

    pub fn auth_entity(&self) -> Entity {
        Entity::asset(self.id.to_string())
    }

    pub fn has_pending_changes(&self) -> bool {
        match self.publication.live_revision() {
            Some(live_revision) => live_revision.id() != self.current_revision.id(),
            None => true,
        }
    }

    pub fn publish_current(&mut self) {
        self.publication.live_revision = Some(self.current_revision.clone());
        self.publication.status = PublicationStatus::Published;
    }

    pub fn apply_publication_transition(
        &mut self,
        transition: PublicationTransition,
    ) -> Result<(), AssetModelError> {
        match transition {
            PublicationTransition::PublishCurrent => {
                self.publish_current();
                Ok(())
            }
            PublicationTransition::Unpublish => self.unpublish(),
        }
    }

    pub fn replace_current_revision(&mut self, revision: ManagedAssetRevision) {
        self.current_revision = revision;
    }

    pub fn unpublish(&mut self) -> Result<(), AssetModelError> {
        if self.publication.live_revision.is_none() {
            return Err(AssetModelError::CannotUnpublishWithoutLiveRevision {
                asset_id: self.id.to_string(),
            });
        }

        self.publication.live_revision = None;
        self.publication.status = PublicationStatus::Unpublished;
        Ok(())
    }

    pub fn plan_public_delivery(
        &self,
        context: &DeliveryContext<'_>,
    ) -> Result<AssetDeliveryPlan, AssetModelError> {
        if !self.publication.is_published() {
            return Err(AssetModelError::NotPublished {
                asset_id: self.id.to_string(),
            });
        }

        let live_revision = self.publication.live_revision().ok_or_else(|| {
            AssetModelError::MissingLiveRevision {
                asset_id: self.id.to_string(),
            }
        })?;

        public_delivery_plan(
            AssetKind::ManagedAsset,
            live_revision.storage_plan(),
            Some(live_revision.id().clone()),
            context,
            false,
        )
        .map_err(|error| match error {
            AssetModelError::PublicDeliveryRequiresPublicCdn { .. } => {
                AssetModelError::PublicDeliveryRequiresPublicCdn {
                    asset_id: self.id.to_string(),
                    delivery_mode: live_revision.storage_plan().policy.delivery_mode,
                }
            }
            other => other,
        })
    }

    pub fn plan_authorized_delivery(
        &self,
        context: &DeliveryContext<'_>,
    ) -> Result<AssetDeliveryPlan, AssetModelError> {
        authorized_delivery_plan(
            AssetKind::ManagedAsset,
            self.current_revision.storage_plan(),
            Some(self.current_revision.id().clone()),
            context,
        )
    }

    pub fn auth_updates(&self) -> Vec<DefaultTupleUpdate> {
        let public_tuple = DefaultTuple::new(
            self.auth_entity(),
            Relation::ReadPublic,
            DefaultSubject::entity(Entity::any_user()),
        );

        if self.publication.is_published()
            && self
                .publication
                .live_revision()
                .is_some_and(|revision| revision.storage_plan().public_delivery_eligible())
        {
            vec![DefaultTupleUpdate::Write(public_tuple)]
        } else {
            vec![DefaultTupleUpdate::Delete(public_tuple)]
        }
    }
}

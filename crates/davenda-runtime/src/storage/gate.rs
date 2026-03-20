use davenda_assets::ManagedAsset;
use davenda_auth::{AuthModelPackage, Capability, DavendaAuth, DefaultSubject};
use zanzibar::RebacEngine;

use super::RuntimeStorageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedAssetPublicationGate {
    pub can_publish: bool,
    pub can_replace: bool,
    pub can_manage_storage: bool,
    pub public_delivery_enabled: bool,
}

impl ManagedAssetPublicationGate {
    pub fn can_publish_publicly(&self) -> bool {
        self.can_publish
            && self.can_replace
            && self.can_manage_storage
            && self.public_delivery_enabled
    }

    pub fn ensure_public_delivery_allowed(
        &self,
        asset_id: impl Into<String>,
    ) -> Result<(), RuntimeStorageError> {
        if self.can_publish_publicly() {
            Ok(())
        } else {
            Err(RuntimeStorageError::PublicationAuthorizationDenied {
                asset_id: asset_id.into(),
                reason: self.denial_reason(),
            })
        }
    }

    pub async fn resolve<E>(
        auth: &DavendaAuth<E>,
        package: &impl AuthModelPackage,
        subject: &DefaultSubject,
        asset: &ManagedAsset,
    ) -> Result<Self, RuntimeStorageError>
    where
        E: RebacEngine,
    {
        let asset_entity = asset.auth_entity();
        let can_publish = check_capability(
            auth,
            package,
            subject,
            asset,
            Capability::AssetPublish,
            &asset_entity,
        )
        .await?;
        let can_replace = check_capability(
            auth,
            package,
            subject,
            asset,
            Capability::AssetReplace,
            &asset_entity,
        )
        .await?;
        let can_manage_storage = check_capability(
            auth,
            package,
            subject,
            asset,
            Capability::AssetManageStorage,
            &asset_entity,
        )
        .await?;

        Ok(Self {
            can_publish,
            can_replace,
            can_manage_storage,
            public_delivery_enabled: asset.publication().is_published()
                && asset
                    .publication()
                    .live_revision()
                    .is_some_and(|revision| revision.storage_plan().public_delivery_eligible()),
        })
    }

    fn denial_reason(&self) -> String {
        let mut missing = Vec::new();
        if !self.can_publish {
            missing.push(Capability::AssetPublish.to_string());
        }
        if !self.can_replace {
            missing.push(Capability::AssetReplace.to_string());
        }
        if !self.can_manage_storage {
            missing.push(Capability::AssetManageStorage.to_string());
        }
        if !self.public_delivery_enabled {
            missing.push("published public delivery state".to_string());
        }
        missing.join(", ")
    }
}

async fn check_capability<E>(
    auth: &DavendaAuth<E>,
    package: &impl AuthModelPackage,
    subject: &DefaultSubject,
    asset: &ManagedAsset,
    capability: Capability,
    asset_entity: &davenda_auth::Entity,
) -> Result<bool, RuntimeStorageError>
where
    E: RebacEngine,
{
    auth.check_capability(package, subject, capability, asset_entity)
        .await
        .map_err(|_| RuntimeStorageError::PublicationAuthorizationDenied {
            asset_id: asset.id().to_string(),
            reason: capability.to_string(),
        })
}

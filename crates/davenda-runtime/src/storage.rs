use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeStorageError {
    #[error(transparent)]
    Storage(#[from] StoragePlanningError),
    #[error(transparent)]
    Asset(#[from] AssetModelError),
    #[error("assets.cdn_base_url must be configured for public asset publication")]
    MissingCdnBaseUrl,
}

#[derive(Debug, Clone)]
pub struct StorageHost {
    pub customer_app: String,
    pub planner: StoragePlanner,
    cdn_base_url: Option<String>,
}

impl StorageHost {
    pub(crate) fn new(
        customer_app: String,
        planner: StoragePlanner,
        cdn_base_url: Option<String>,
    ) -> Self {
        Self {
            customer_app,
            planner,
            cdn_base_url,
        }
    }

    pub fn plan_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, RuntimeStorageError> {
        Ok(self.planner.plan_write(request)?)
    }

    pub fn publish_deployment_release(
        &self,
        release: &DeploymentRelease,
    ) -> Result<ActiveAssetManifest, RuntimeStorageError> {
        let cdn_base_url = self
            .cdn_base_url
            .as_deref()
            .ok_or(RuntimeStorageError::MissingCdnBaseUrl)?;
        Ok(release.publish(&self.planner, cdn_base_url)?)
    }

    pub fn plan_managed_revision(
        &self,
        revision_id: RevisionId,
        logical_path: impl Into<String>,
        override_policy: Option<StoragePolicyOverride>,
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<ManagedAssetRevision, RuntimeStorageError> {
        Ok(ManagedAssetRevision::plan(
            revision_id,
            &self.planner,
            logical_path,
            override_policy,
            content_type,
            byte_length,
            fingerprint,
        )?)
    }

    pub fn plan_public_asset_delivery(
        &self,
        asset: &ManagedAsset,
    ) -> Result<AssetDeliveryPlan, RuntimeStorageError> {
        let cdn_base_url = self
            .cdn_base_url
            .as_deref()
            .ok_or(RuntimeStorageError::MissingCdnBaseUrl)?;
        let context = DeliveryContext::default().with_cdn_base_url(cdn_base_url);
        Ok(asset.plan_public_delivery(&context)?)
    }

    pub fn plan_authorized_asset_delivery(
        &self,
        asset: &ManagedAsset,
    ) -> Result<AssetDeliveryPlan, RuntimeStorageError> {
        Ok(asset.plan_authorized_delivery(&DeliveryContext::default())?)
    }
}

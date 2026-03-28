use coil_assets::{
    ActiveAssetManifest, AssetDeliveryPlan, ContentFingerprint, DeliveryContext, DeploymentRelease,
    ManagedAsset, ManagedAssetRevision, RevisionId, ThemeAssetPublicationPlan,
    ThemeAssetPublicationReceipt,
};
use coil_auth::{AuthModelPackage, CoilAuth, DefaultSubject};
use coil_storage::{
    SingleNodeEscapeHatchPlanner, StoragePlan, StoragePlanRequest, StoragePlanner,
    StoragePolicyOverride,
    execution::{
        ObjectStoreClientConfig, StorageDeliveryLocation, StorageExecutor, StorageReadReceipt,
        StorageWriteReceipt,
    },
};
use zanzibar::RebacEngine;

use super::{ManagedAssetPublicationGate, RuntimeStorageError};

#[derive(Debug, Clone)]
pub struct StorageHost {
    pub customer_app: String,
    pub planner: StoragePlanner,
    executor: StorageExecutor,
    single_node_escape_hatch: SingleNodeEscapeHatchPlanner,
    cdn_base_url: Option<String>,
}

impl StorageHost {
    pub(crate) fn new(
        customer_app: String,
        planner: StoragePlanner,
        cdn_base_url: Option<String>,
        object_store: Option<ObjectStoreClientConfig>,
    ) -> Self {
        let executor =
            StorageExecutor::from_topology_and_object_store(planner.topology(), object_store);
        Self {
            customer_app,
            single_node_escape_hatch: planner.single_node_escape_hatch(),
            planner,
            executor,
            cdn_base_url,
        }
    }

    pub fn plan_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, RuntimeStorageError> {
        Ok(self.planner.plan_scalable_write(request)?)
    }

    pub fn plan_single_node_escape_hatch_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, RuntimeStorageError> {
        Ok(self.single_node_escape_hatch.plan_write(request)?)
    }

    pub fn execute_write(
        &self,
        plan: &StoragePlan,
        bytes: impl AsRef<[u8]>,
    ) -> Result<StorageWriteReceipt, RuntimeStorageError> {
        Ok(self.executor.execute_write(plan, bytes)?)
    }

    pub fn execute_write_with_content_type(
        &self,
        plan: &StoragePlan,
        bytes: impl AsRef<[u8]>,
        content_type: Option<&str>,
    ) -> Result<StorageWriteReceipt, RuntimeStorageError> {
        Ok(self
            .executor
            .execute_write_with_content_type(plan, bytes, content_type)?)
    }

    pub fn execute_read(
        &self,
        plan: &StoragePlan,
    ) -> Result<StorageReadReceipt, RuntimeStorageError> {
        Ok(self.executor.execute_read(plan)?)
    }

    pub fn delivery_location(
        &self,
        plan: &StoragePlan,
    ) -> Result<StorageDeliveryLocation, RuntimeStorageError> {
        Ok(self
            .executor
            .delivery_location(plan, self.cdn_base_url.as_deref())?)
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

    pub fn publish_theme_assets(
        &self,
        publication: &ThemeAssetPublicationPlan,
    ) -> Result<ThemeAssetPublicationReceipt, RuntimeStorageError> {
        let cdn_base_url = self
            .cdn_base_url
            .as_deref()
            .ok_or(RuntimeStorageError::MissingCdnBaseUrl)?;
        Ok(publication.publish_and_sync(&self.planner, cdn_base_url, &self.executor)?)
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

    pub fn plan_managed_revision_with_single_node_escape_hatch(
        &self,
        revision_id: RevisionId,
        logical_path: impl Into<String>,
        override_policy: Option<StoragePolicyOverride>,
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<ManagedAssetRevision, RuntimeStorageError> {
        Ok(ManagedAssetRevision::plan_with_single_node_escape_hatch(
            revision_id,
            &self.single_node_escape_hatch,
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

    pub async fn managed_asset_publication_gate<E>(
        &self,
        auth: &CoilAuth<E>,
        package: &impl AuthModelPackage,
        subject: &DefaultSubject,
        asset: &ManagedAsset,
    ) -> Result<ManagedAssetPublicationGate, RuntimeStorageError>
    where
        E: RebacEngine,
    {
        ManagedAssetPublicationGate::resolve(auth, package, subject, asset).await
    }
}

use super::*;
use davenda_assets::ManagedAsset;
use davenda_auth::{AuthModelPackage, Capability, DavendaAuth, DefaultSubject};
use zanzibar::RebacEngine;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeStorageError {
    #[error(transparent)]
    Storage(#[from] StoragePlanningError),
    #[error(transparent)]
    Execution(#[from] StorageExecutionError),
    #[error(transparent)]
    Asset(#[from] AssetModelError),
    #[error("assets.cdn_base_url must be configured for public asset publication")]
    MissingCdnBaseUrl,
    #[error("asset publication authorization failed for `{asset_id}`: {reason}")]
    PublicationAuthorizationDenied { asset_id: String, reason: String },
}

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
    ) -> Self {
        let executor = StorageExecutor::from_topology(planner.topology());
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
        auth: &DavendaAuth<E>,
        package: &impl AuthModelPackage,
        subject: &DefaultSubject,
        asset: &ManagedAsset,
    ) -> Result<ManagedAssetPublicationGate, RuntimeStorageError>
    where
        E: RebacEngine,
    {
        let asset_entity = asset.auth_entity();
        let can_publish = auth
            .check_capability(package, subject, Capability::AssetPublish, &asset_entity)
            .await
            .map_err(|_| RuntimeStorageError::PublicationAuthorizationDenied {
                asset_id: asset.id().to_string(),
                reason: Capability::AssetPublish.to_string(),
            })?;
        let can_replace = auth
            .check_capability(package, subject, Capability::AssetReplace, &asset_entity)
            .await
            .map_err(|_| RuntimeStorageError::PublicationAuthorizationDenied {
                asset_id: asset.id().to_string(),
                reason: Capability::AssetReplace.to_string(),
            })?;
        let can_manage_storage = auth
            .check_capability(
                package,
                subject,
                Capability::AssetManageStorage,
                &asset_entity,
            )
            .await
            .map_err(|_| RuntimeStorageError::PublicationAuthorizationDenied {
                asset_id: asset.id().to_string(),
                reason: Capability::AssetManageStorage.to_string(),
            })?;

        Ok(ManagedAssetPublicationGate {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_config::{PlatformConfig, StorageClass};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn publication_gate_reports_missing_conditions() {
        let gate = ManagedAssetPublicationGate {
            can_publish: true,
            can_replace: false,
            can_manage_storage: true,
            public_delivery_enabled: false,
        };

        assert!(!gate.can_publish_publicly());
        let error = gate
            .ensure_public_delivery_allowed("asset-hero")
            .unwrap_err();
        assert_eq!(
            error,
            RuntimeStorageError::PublicationAuthorizationDenied {
                asset_id: "asset-hero".to_string(),
                reason: "asset.replace, published public delivery state".to_string(),
            }
        );
    }

    fn test_config() -> PlatformConfig {
        PlatformConfig::from_toml_str(
            r#"
[app]
name = "davenda-runtime-storage-tests"
environment = "development"

[server]
bind = "127.0.0.1:3000"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
local_root = "/tmp/davenda-runtime-storage-tests"
deployment = "single_node"

[cache]
l1 = "moka"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = []

[wasm]
directory = "/tmp/davenda-runtime-storage-tests"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#,
        )
        .unwrap()
    }

    #[test]
    fn storage_host_executes_object_store_and_local_storage_plans() {
        let root = PathBuf::from("/tmp/davenda-runtime-storage-tests");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let plan = RuntimeBuilder::new(test_config(), DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let host = plan.storage_host();

        let object_plan = host
            .plan_write(
                StoragePlanRequest::new("uploads/catalog/item.jpg")
                    .with_storage_class(StorageClass::PublicUpload),
            )
            .unwrap();
        let object_write = host.execute_write(&object_plan, b"object-bytes").unwrap();
        assert_eq!(object_write.bytes_written, "object-bytes".len() as u64);
        assert!(
            object_write
                .path
                .ends_with("object-store/uploads/catalog/item.jpg")
        );
        assert_eq!(
            host.execute_read(&object_plan).unwrap().bytes,
            b"object-bytes"
        );
        assert!(matches!(
            host.delivery_location(&object_plan).unwrap(),
            StorageDeliveryLocation::PublicCdn { .. }
        ));

        let local_plan = host
            .plan_single_node_escape_hatch_write(
                StoragePlanRequest::new("secure/reports/march.csv")
                    .with_storage_class(StorageClass::PrivateShared)
                    .with_override(StoragePolicyOverride::force_single_node_escape_hatch()),
            )
            .unwrap();
        let local_write = host.execute_write(&local_plan, b"local-bytes").unwrap();
        assert_eq!(
            local_write.path,
            PathBuf::from("/tmp/davenda-runtime-storage-tests/secure/reports/march.csv")
        );
        assert_eq!(
            host.execute_read(&local_plan).unwrap().bytes,
            b"local-bytes"
        );
        assert_eq!(
            host.delivery_location(&local_plan).unwrap(),
            StorageDeliveryLocation::LocalPath {
                path: PathBuf::from("/tmp/davenda-runtime-storage-tests/secure/reports/march.csv"),
            }
        );

        let _ = fs::remove_dir_all(root);
    }
}

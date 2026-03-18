use std::collections::{BTreeMap, btree_map::Entry};
use std::fmt;

use davenda_storage::{
    DeliveryMode, DurableStore, Sensitivity, StoragePlan, StoragePlanRequest, StoragePlanner,
    StoragePlanningError, StoragePolicy, StoragePolicyOverride, SyncMode,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    DeploymentArtifact,
    ManagedAsset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryAudience {
    Public,
    Authorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FingerprintAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl fmt::Display for FingerprintAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256 => f.write_str("sha256"),
            Self::Sha384 => f.write_str("sha384"),
            Self::Sha512 => f.write_str("sha512"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentFingerprint {
    algorithm: FingerprintAlgorithm,
    digest: String,
}

impl ContentFingerprint {
    pub fn new(
        algorithm: FingerprintAlgorithm,
        digest: impl Into<String>,
    ) -> Result<Self, AssetModelError> {
        Ok(Self {
            algorithm,
            digest: require_non_empty("digest", digest.into())?,
        })
    }

    pub fn algorithm(&self) -> FingerprintAlgorithm {
        self.algorithm
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(String);

impl AssetId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("asset_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("revision_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseId(String);

impl ReleaseId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("release_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReleaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliveryContext<'a> {
    pub cdn_base_url: Option<&'a str>,
    pub app_proxy_base: Option<&'a str>,
}

impl<'a> DeliveryContext<'a> {
    pub fn with_cdn_base_url(mut self, base_url: &'a str) -> Self {
        self.cdn_base_url = Some(base_url);
        self
    }

    pub fn with_app_proxy_base(mut self, base_path: &'a str) -> Self {
        self.app_proxy_base = Some(base_path);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetDeliveryTarget {
    Cdn {
        public_url: String,
        object_key: String,
    },
    SignedObject {
        object_key: String,
    },
    AppProxy {
        path: String,
    },
    LocalPath {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDeliveryPlan {
    asset_kind: AssetKind,
    audience: DeliveryAudience,
    storage_plan: StoragePlan,
    revision_id: Option<RevisionId>,
    target: AssetDeliveryTarget,
    immutable: bool,
}

impl AssetDeliveryPlan {
    pub fn asset_kind(&self) -> AssetKind {
        self.asset_kind
    }

    pub fn audience(&self) -> DeliveryAudience {
        self.audience
    }

    pub fn storage_plan(&self) -> &StoragePlan {
        &self.storage_plan
    }

    pub fn revision_id(&self) -> Option<&RevisionId> {
        self.revision_id.as_ref()
    }

    pub fn target(&self) -> &AssetDeliveryTarget {
        &self.target
    }

    pub fn immutable(&self) -> bool {
        self.immutable
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        self.storage_plan.policy.delivery_mode
    }

    pub fn durable_store(&self) -> DurableStore {
        self.storage_plan.durable_store
    }

    pub fn sensitivity(&self) -> Sensitivity {
        self.storage_plan.policy.sensitivity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentArtifact {
    logical_path: String,
    hashed_path: String,
    fingerprint: ContentFingerprint,
    content_type: String,
    byte_length: u64,
}

impl DeploymentArtifact {
    pub fn new(
        logical_path: impl Into<String>,
        hashed_path: impl Into<String>,
        fingerprint: ContentFingerprint,
        content_type: impl Into<String>,
        byte_length: u64,
    ) -> Result<Self, AssetModelError> {
        let logical_path = normalize_manifest_path("logical_path", logical_path.into())?;
        let hashed_path = normalize_manifest_path("hashed_path", hashed_path.into())?;
        let content_type = require_non_empty("content_type", content_type.into())?;

        if logical_path == hashed_path || !hashed_path.contains(fingerprint.digest()) {
            return Err(AssetModelError::UnhashedDeploymentArtifact {
                logical_path,
                hashed_path,
                fingerprint: fingerprint.digest().to_string(),
            });
        }

        Ok(Self {
            logical_path,
            hashed_path,
            fingerprint,
            content_type,
            byte_length,
        })
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub fn hashed_path(&self) -> &str {
        &self.hashed_path
    }

    pub fn fingerprint(&self) -> &ContentFingerprint {
        &self.fingerprint
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedDeploymentArtifact {
    artifact: DeploymentArtifact,
    delivery: AssetDeliveryPlan,
}

impl PublishedDeploymentArtifact {
    pub fn artifact(&self) -> &DeploymentArtifact {
        &self.artifact
    }

    pub fn delivery(&self) -> &AssetDeliveryPlan {
        &self.delivery
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAssetManifest {
    release_id: ReleaseId,
    entries: BTreeMap<String, PublishedDeploymentArtifact>,
}

impl ActiveAssetManifest {
    pub fn release_id(&self) -> &ReleaseId {
        &self.release_id
    }

    pub fn resolve(&self, logical_path: &str) -> Option<&PublishedDeploymentArtifact> {
        self.entries.get(logical_path)
    }

    pub fn entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &PublishedDeploymentArtifact)> + '_ {
        self.entries
            .iter()
            .map(|(path, artifact)| (path.as_str(), artifact))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentRelease {
    release_id: ReleaseId,
    artifacts: Vec<DeploymentArtifact>,
}

impl DeploymentRelease {
    pub fn new(
        release_id: ReleaseId,
        artifacts: impl IntoIterator<Item = DeploymentArtifact>,
    ) -> Result<Self, AssetModelError> {
        let artifacts = artifacts.into_iter().collect::<Vec<_>>();
        if artifacts.is_empty() {
            return Err(AssetModelError::EmptyField {
                field: "deployment_artifacts",
            });
        }

        Ok(Self {
            release_id,
            artifacts,
        })
    }

    pub fn release_id(&self) -> &ReleaseId {
        &self.release_id
    }

    pub fn artifacts(&self) -> &[DeploymentArtifact] {
        &self.artifacts
    }

    pub fn publish(
        &self,
        planner: &StoragePlanner,
        cdn_base_url: &str,
    ) -> Result<ActiveAssetManifest, AssetModelError> {
        let context = DeliveryContext::default().with_cdn_base_url(cdn_base_url);
        let mut entries = BTreeMap::new();

        for artifact in &self.artifacts {
            let storage_plan = planner
                .plan_write(
                    StoragePlanRequest::new(artifact.hashed_path())
                        .with_override(public_deployment_override()),
                )
                .map_err(AssetModelError::Storage)?;

            if storage_plan.policy != StoragePolicy::public_asset() {
                return Err(AssetModelError::InvalidDeploymentPolicy {
                    logical_path: artifact.logical_path().to_string(),
                    policy: storage_plan.policy,
                });
            }

            let delivery = public_delivery_plan(
                AssetKind::DeploymentArtifact,
                &storage_plan,
                None,
                &context,
                true,
            )?;

            match entries.entry(artifact.logical_path().to_string()) {
                Entry::Vacant(entry) => {
                    entry.insert(PublishedDeploymentArtifact {
                        artifact: artifact.clone(),
                        delivery,
                    });
                }
                Entry::Occupied(_) => {
                    return Err(AssetModelError::DuplicateDeploymentArtifact {
                        release_id: self.release_id.to_string(),
                        logical_path: artifact.logical_path().to_string(),
                    });
                }
            }
        }

        Ok(ActiveAssetManifest {
            release_id: self.release_id.clone(),
            entries,
        })
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationState {
    status: PublicationStatus,
    live_revision: Option<ManagedAssetRevision>,
}

impl PublicationState {
    pub fn status(&self) -> PublicationStatus {
        self.status
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
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssetModelError {
    #[error(transparent)]
    Storage(#[from] StoragePlanningError),
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error(
        "deployment artifact `{logical_path}` must be content-addressed and include fingerprint `{fingerprint}` in `{hashed_path}`"
    )]
    UnhashedDeploymentArtifact {
        logical_path: String,
        hashed_path: String,
        fingerprint: String,
    },
    #[error("deployment release `{release_id}` contains duplicate logical path `{logical_path}`")]
    DuplicateDeploymentArtifact {
        release_id: String,
        logical_path: String,
    },
    #[error(
        "deployment artifact `{logical_path}` resolved to policy {policy:?}; deployment artifacts must remain public object-store assets"
    )]
    InvalidDeploymentPolicy {
        logical_path: String,
        policy: StoragePolicy,
    },
    #[error("asset `{asset_id}` has no live published revision")]
    MissingLiveRevision { asset_id: String },
    #[error("asset `{asset_id}` cannot be delivered publicly with mode `{delivery_mode}`")]
    PublicDeliveryRequiresPublicCdn {
        asset_id: String,
        delivery_mode: DeliveryMode,
    },
    #[error("delivery planning for `{logical_path}` requires an object-store key")]
    MissingObjectKey { logical_path: String },
    #[error("delivery planning for `{logical_path}` requires a local path")]
    MissingLocalPath { logical_path: String },
    #[error("delivery planning for `{logical_path}` requires a CDN base URL")]
    MissingCdnBaseUrl { logical_path: String },
    #[error("delivery planning for `{logical_path}` requires an app proxy base path")]
    MissingAppProxyBase { logical_path: String },
    #[error("asset `{asset_id}` has no live revision to unpublish")]
    CannotUnpublishWithoutLiveRevision { asset_id: String },
}

fn public_deployment_override() -> StoragePolicyOverride {
    StoragePolicyOverride {
        delivery_mode: Some(DeliveryMode::PublicCdn),
        sync_mode: Some(SyncMode::ObjectStore),
        sensitivity: Some(Sensitivity::Public),
    }
}

fn public_delivery_plan(
    asset_kind: AssetKind,
    storage_plan: &StoragePlan,
    revision_id: Option<RevisionId>,
    context: &DeliveryContext<'_>,
    immutable: bool,
) -> Result<AssetDeliveryPlan, AssetModelError> {
    if storage_plan.policy.delivery_mode != DeliveryMode::PublicCdn {
        return Err(AssetModelError::PublicDeliveryRequiresPublicCdn {
            asset_id: revision_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| storage_plan.logical_path.clone()),
            delivery_mode: storage_plan.policy.delivery_mode,
        });
    }

    Ok(AssetDeliveryPlan {
        asset_kind,
        audience: DeliveryAudience::Public,
        storage_plan: storage_plan.clone(),
        revision_id,
        target: AssetDeliveryTarget::Cdn {
            public_url: join_delivery_base(
                context
                    .cdn_base_url
                    .ok_or_else(|| AssetModelError::MissingCdnBaseUrl {
                        logical_path: storage_plan.logical_path.clone(),
                    })?,
                storage_plan.object_key.as_ref().ok_or_else(|| {
                    AssetModelError::MissingObjectKey {
                        logical_path: storage_plan.logical_path.clone(),
                    }
                })?,
            ),
            object_key: storage_plan.object_key.clone().ok_or_else(|| {
                AssetModelError::MissingObjectKey {
                    logical_path: storage_plan.logical_path.clone(),
                }
            })?,
        },
        immutable,
    })
}

fn authorized_delivery_plan(
    asset_kind: AssetKind,
    storage_plan: &StoragePlan,
    revision_id: Option<RevisionId>,
    context: &DeliveryContext<'_>,
) -> Result<AssetDeliveryPlan, AssetModelError> {
    let target = match storage_plan.policy.delivery_mode {
        DeliveryMode::PublicCdn => AssetDeliveryTarget::Cdn {
            public_url: join_delivery_base(
                context
                    .cdn_base_url
                    .ok_or_else(|| AssetModelError::MissingCdnBaseUrl {
                        logical_path: storage_plan.logical_path.clone(),
                    })?,
                storage_plan.object_key.as_ref().ok_or_else(|| {
                    AssetModelError::MissingObjectKey {
                        logical_path: storage_plan.logical_path.clone(),
                    }
                })?,
            ),
            object_key: storage_plan.object_key.clone().ok_or_else(|| {
                AssetModelError::MissingObjectKey {
                    logical_path: storage_plan.logical_path.clone(),
                }
            })?,
        },
        DeliveryMode::SignedUrl => AssetDeliveryTarget::SignedObject {
            object_key: storage_plan.object_key.clone().ok_or_else(|| {
                AssetModelError::MissingObjectKey {
                    logical_path: storage_plan.logical_path.clone(),
                }
            })?,
        },
        DeliveryMode::AppProxy => AssetDeliveryTarget::AppProxy {
            path: join_delivery_base(
                context
                    .app_proxy_base
                    .ok_or_else(|| AssetModelError::MissingAppProxyBase {
                        logical_path: storage_plan.logical_path.clone(),
                    })?,
                &storage_plan.logical_path,
            ),
        },
        DeliveryMode::LocalOnly => AssetDeliveryTarget::LocalPath {
            path: storage_plan.local_path.clone().ok_or_else(|| {
                AssetModelError::MissingLocalPath {
                    logical_path: storage_plan.logical_path.clone(),
                }
            })?,
        },
    };

    Ok(AssetDeliveryPlan {
        asset_kind,
        audience: DeliveryAudience::Authorized,
        storage_plan: storage_plan.clone(),
        revision_id,
        target,
        immutable: false,
    })
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, AssetModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AssetModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_manifest_path(field: &'static str, value: String) -> Result<String, AssetModelError> {
    let value = require_non_empty(field, value)?;
    let normalized = value.trim_matches('/').to_string();
    if normalized.is_empty() {
        Err(AssetModelError::EmptyField { field })
    } else {
        Ok(normalized)
    }
}

fn join_delivery_base(base: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_config::ObjectStoreKind;
    use davenda_storage::{
        ObjectStoreTarget, StorageBackendKind, StoragePolicySet, StorageTopology,
    };

    fn object_store_planner() -> StoragePlanner {
        StoragePlanner::new(
            StorageTopology {
                local_root: "/srv/davenda".to_string(),
                default_class: davenda_config::StorageClass::PublicUpload,
                object_store: Some(ObjectStoreTarget {
                    kind: ObjectStoreKind::S3,
                }),
            },
            StoragePolicySet::default(),
        )
    }

    fn local_only_planner() -> StoragePlanner {
        StoragePlanner::new(
            StorageTopology {
                local_root: "/srv/davenda".to_string(),
                default_class: davenda_config::StorageClass::LocalOnlySensitive,
                object_store: None,
            },
            StoragePolicySet::default(),
        )
    }

    fn fingerprint(seed: &str) -> ContentFingerprint {
        ContentFingerprint::new(FingerprintAlgorithm::Sha256, seed).unwrap()
    }

    #[test]
    fn deployment_release_publishes_hashed_artifacts_to_a_manifest() {
        let planner = object_store_planner();
        let release = DeploymentRelease::new(
            ReleaseId::new("release-20260318").unwrap(),
            [DeploymentArtifact::new(
                "theme/app.css",
                "deploy/theme/app.abc123.css",
                fingerprint("abc123"),
                "text/css",
                4096,
            )
            .unwrap()],
        )
        .unwrap();

        let manifest = release
            .publish(&planner, "https://cdn.example.com/assets")
            .unwrap();
        let entry = manifest.resolve("theme/app.css").unwrap();

        assert_eq!(manifest.release_id().as_str(), "release-20260318");
        assert_eq!(entry.delivery().asset_kind(), AssetKind::DeploymentArtifact);
        assert_eq!(entry.delivery().audience(), DeliveryAudience::Public);
        assert_eq!(entry.delivery().delivery_mode(), DeliveryMode::PublicCdn);
        assert_eq!(entry.delivery().durable_store(), DurableStore::ObjectStore);
        assert!(entry.delivery().immutable());
        assert_eq!(
            entry
                .delivery()
                .storage_plan()
                .primary_write_target()
                .unwrap()
                .backend,
            StorageBackendKind::S3Compatible
        );
        assert_eq!(
            entry.delivery().target(),
            &AssetDeliveryTarget::Cdn {
                public_url: "https://cdn.example.com/assets/deploy/theme/app.abc123.css"
                    .to_string(),
                object_key: "deploy/theme/app.abc123.css".to_string(),
            }
        );
    }

    #[test]
    fn deployment_release_rejects_duplicate_logical_paths() {
        let planner = object_store_planner();
        let release = DeploymentRelease::new(
            ReleaseId::new("release-dup").unwrap(),
            [
                DeploymentArtifact::new(
                    "theme/app.css",
                    "deploy/theme/app.abc123.css",
                    fingerprint("abc123"),
                    "text/css",
                    4096,
                )
                .unwrap(),
                DeploymentArtifact::new(
                    "theme/app.css",
                    "deploy/theme/app.def456.css",
                    fingerprint("def456"),
                    "text/css",
                    8192,
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(
            release
                .publish(&planner, "https://cdn.example.com")
                .unwrap_err(),
            AssetModelError::DuplicateDeploymentArtifact {
                release_id: "release-dup".to_string(),
                logical_path: "theme/app.css".to_string(),
            }
        );
    }

    #[test]
    fn managed_assets_require_publication_before_public_delivery() {
        let planner = object_store_planner();
        let revision = ManagedAssetRevision::plan(
            RevisionId::new("rev-1").unwrap(),
            &planner,
            "media/brochure.pdf",
            Some(StoragePolicyOverride {
                delivery_mode: Some(DeliveryMode::PublicCdn),
                sync_mode: Some(SyncMode::ObjectStore),
                sensitivity: Some(Sensitivity::Public),
            }),
            "application/pdf",
            1024,
            fingerprint("rev1"),
        )
        .unwrap();

        let asset = ManagedAsset::new(
            AssetId::new("asset-brochure").unwrap(),
            "Event brochure",
            revision,
        )
        .unwrap();

        assert_eq!(
            asset
                .plan_public_delivery(
                    &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com")
                )
                .unwrap_err(),
            AssetModelError::MissingLiveRevision {
                asset_id: "asset-brochure".to_string(),
            }
        );
    }

    #[test]
    fn replacing_a_managed_asset_keeps_the_live_revision_until_republished() {
        let planner = object_store_planner();
        let mut asset = ManagedAsset::new(
            AssetId::new("asset-hero").unwrap(),
            "Homepage hero",
            ManagedAssetRevision::plan(
                RevisionId::new("rev-1").unwrap(),
                &planner,
                "media/hero-v1.jpg",
                Some(StoragePolicyOverride {
                    delivery_mode: Some(DeliveryMode::PublicCdn),
                    sync_mode: Some(SyncMode::ObjectStore),
                    sensitivity: Some(Sensitivity::Public),
                }),
                "image/jpeg",
                2048,
                fingerprint("hero1"),
            )
            .unwrap(),
        )
        .unwrap();

        asset.publish_current();
        let first_live = asset
            .plan_public_delivery(
                &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
            )
            .unwrap();
        assert_eq!(
            first_live.revision_id().map(RevisionId::as_str),
            Some("rev-1")
        );

        asset.replace_current_revision(
            ManagedAssetRevision::plan(
                RevisionId::new("rev-2").unwrap(),
                &planner,
                "media/hero-v2.jpg",
                Some(StoragePolicyOverride {
                    delivery_mode: Some(DeliveryMode::PublicCdn),
                    sync_mode: Some(SyncMode::ObjectStore),
                    sensitivity: Some(Sensitivity::Public),
                }),
                "image/jpeg",
                3072,
                fingerprint("hero2"),
            )
            .unwrap(),
        );

        assert!(asset.has_pending_changes());
        let still_live = asset
            .plan_public_delivery(
                &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
            )
            .unwrap();
        assert_eq!(
            still_live.revision_id().map(RevisionId::as_str),
            Some("rev-1")
        );

        asset.publish_current();
        let republished = asset
            .plan_public_delivery(
                &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
            )
            .unwrap();
        assert_eq!(
            republished.revision_id().map(RevisionId::as_str),
            Some("rev-2")
        );
    }

    #[test]
    fn private_assets_plan_authorized_delivery_from_storage_policy() {
        let planner = local_only_planner();
        let revision = ManagedAssetRevision::plan(
            RevisionId::new("rev-local").unwrap(),
            &planner,
            "staff/exports/orders.csv",
            Some(StoragePolicyOverride::force_local_only()),
            "text/csv",
            512,
            fingerprint("orders1"),
        )
        .unwrap();

        let asset = ManagedAsset::new(
            AssetId::new("asset-orders-export").unwrap(),
            "Orders export",
            revision,
        )
        .unwrap();

        let plan = asset
            .plan_authorized_delivery(
                &DeliveryContext::default().with_app_proxy_base("/admin/downloads"),
            )
            .unwrap();

        assert_eq!(plan.asset_kind(), AssetKind::ManagedAsset);
        assert_eq!(plan.audience(), DeliveryAudience::Authorized);
        assert_eq!(plan.delivery_mode(), DeliveryMode::LocalOnly);
        assert_eq!(plan.durable_store(), DurableStore::LocalDisk);
        assert_eq!(plan.sensitivity(), Sensitivity::Secret);
        assert_eq!(
            plan.target(),
            &AssetDeliveryTarget::LocalPath {
                path: "srv/davenda/staff/exports/orders.csv".to_string(),
            }
        );
    }
}

use std::collections::{btree_map::Entry, BTreeMap};
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

mod managed;
mod release;
pub use managed::{ManagedAsset, ManagedAssetRevision, PublicationState, PublicationStatus};
pub use release::{
    ActiveAssetManifest, DeploymentArtifact, DeploymentRelease, PublishedDeploymentArtifact,
};

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
mod tests;

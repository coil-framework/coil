use std::fmt;

use coil_storage::{
    DeliveryMode, DurableStore, Sensitivity, StoragePlan, StoragePolicyOverride, SyncMode,
};

use crate::{AssetModelError, RevisionId, join_delivery_base, require_non_empty};

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

pub fn public_deployment_override() -> StoragePolicyOverride {
    StoragePolicyOverride {
        delivery_mode: Some(DeliveryMode::PublicCdn),
        sync_mode: Some(SyncMode::ObjectStore),
        sensitivity: Some(Sensitivity::Public),
    }
}

pub fn public_delivery_plan(
    asset_kind: AssetKind,
    storage_plan: &StoragePlan,
    revision_id: Option<RevisionId>,
    context: &DeliveryContext<'_>,
    immutable: bool,
) -> Result<AssetDeliveryPlan, AssetModelError> {
    storage_plan
        .ensure_public_delivery_allowed()
        .map_err(|error| match error {
            coil_storage::StoragePlanningError::PublicDeliveryNotEligible { policy, .. } => {
                AssetModelError::PublicDeliveryRequiresPublicCdn {
                    asset_id: revision_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| storage_plan.logical_path.clone()),
                    delivery_mode: policy.delivery_mode,
                }
            }
            other => AssetModelError::Storage(other),
        })?;

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

pub fn authorized_delivery_plan(
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

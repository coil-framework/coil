use std::fmt;

use coil_config::StorageClass;

use super::StoragePolicyError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageBackendKind {
    LocalDisk,
    S3Compatible,
}

impl fmt::Display for StorageBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalDisk => f.write_str("local_disk"),
            Self::S3Compatible => f.write_str("s3_compatible"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathPolicyKind {
    Folder,
    Upload,
}

impl fmt::Display for PathPolicyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Folder => f.write_str("folder"),
            Self::Upload => f.write_str("upload"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurableStore {
    LocalDisk,
    ObjectStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

impl fmt::Display for DeliveryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicCdn => f.write_str("public_cdn"),
            Self::SignedUrl => f.write_str("signed_url"),
            Self::AppProxy => f.write_str("app_proxy"),
            Self::LocalOnly => f.write_str("local_only"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncMode {
    ObjectStore,
    LocalOnly,
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectStore => f.write_str("object_store"),
            Self::LocalOnly => f.write_str("local_only"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sensitivity {
    Public,
    Internal,
    Restricted,
    Secret,
}

impl fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Internal => f.write_str("internal"),
            Self::Restricted => f.write_str("restricted"),
            Self::Secret => f.write_str("secret"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoragePolicy {
    pub delivery_mode: DeliveryMode,
    pub sync_mode: SyncMode,
    pub sensitivity: Sensitivity,
}

impl StoragePolicy {
    pub const fn new(
        delivery_mode: DeliveryMode,
        sync_mode: SyncMode,
        sensitivity: Sensitivity,
    ) -> Self {
        Self {
            delivery_mode,
            sync_mode,
            sensitivity,
        }
    }

    pub const fn public_asset() -> Self {
        Self::new(
            DeliveryMode::PublicCdn,
            SyncMode::ObjectStore,
            Sensitivity::Public,
        )
    }

    pub const fn public_upload() -> Self {
        Self::new(
            DeliveryMode::PublicCdn,
            SyncMode::ObjectStore,
            Sensitivity::Public,
        )
    }

    pub const fn private_shared() -> Self {
        Self::new(
            DeliveryMode::SignedUrl,
            SyncMode::ObjectStore,
            Sensitivity::Restricted,
        )
    }

    pub const fn single_node_sensitive() -> Self {
        Self::new(
            DeliveryMode::LocalOnly,
            SyncMode::LocalOnly,
            Sensitivity::Secret,
        )
    }

    pub fn validate(&self) -> Result<(), StoragePolicyError> {
        match (self.delivery_mode, self.sync_mode, self.sensitivity) {
            (DeliveryMode::PublicCdn, SyncMode::LocalOnly, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "public_cdn delivery requires object_store sync".to_string(),
                })
            }
            (DeliveryMode::SignedUrl, SyncMode::LocalOnly, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "signed_url delivery requires object_store sync".to_string(),
                })
            }
            (
                DeliveryMode::PublicCdn,
                _,
                Sensitivity::Internal | Sensitivity::Restricted | Sensitivity::Secret,
            ) => Err(StoragePolicyError::InvalidCombination {
                detail: "public_cdn delivery is only valid for public content".to_string(),
            }),
            (DeliveryMode::LocalOnly, SyncMode::ObjectStore, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "local_only delivery cannot use object_store sync".to_string(),
                })
            }
            (DeliveryMode::SignedUrl, _, Sensitivity::Public) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "signed_url delivery is for non-public content".to_string(),
                })
            }
            (_, _, _) => Ok(()),
        }
    }

    pub const fn durable_store(&self) -> DurableStore {
        match self.sync_mode {
            SyncMode::ObjectStore => DurableStore::ObjectStore,
            SyncMode::LocalOnly => DurableStore::LocalDisk,
        }
    }

    pub const fn is_public_delivery_eligible(&self) -> bool {
        matches!(
            (self.delivery_mode, self.sensitivity),
            (DeliveryMode::PublicCdn, Sensitivity::Public)
        )
    }
}

impl From<StorageClass> for StoragePolicy {
    fn from(value: StorageClass) -> Self {
        match value {
            StorageClass::PublicAsset => Self::public_asset(),
            StorageClass::PublicUpload => Self::public_upload(),
            StorageClass::PrivateShared => Self::private_shared(),
            StorageClass::LocalOnlySensitive => Self::single_node_sensitive(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoragePolicyOverride {
    pub delivery_mode: Option<DeliveryMode>,
    pub sync_mode: Option<SyncMode>,
    pub sensitivity: Option<Sensitivity>,
}

impl StoragePolicyOverride {
    pub fn apply_to(&self, base: StoragePolicy) -> StoragePolicy {
        StoragePolicy {
            delivery_mode: self.delivery_mode.unwrap_or(base.delivery_mode),
            sync_mode: self.sync_mode.unwrap_or(base.sync_mode),
            sensitivity: self.sensitivity.unwrap_or(base.sensitivity),
        }
    }

    pub const fn is_local_only_escape_hatch(&self) -> bool {
        matches!(
            (self.delivery_mode, self.sync_mode, self.sensitivity,),
            (
                Some(DeliveryMode::LocalOnly),
                Some(SyncMode::LocalOnly),
                Some(Sensitivity::Secret),
            )
        )
    }

    pub fn force_single_node_escape_hatch() -> Self {
        Self {
            delivery_mode: Some(DeliveryMode::LocalOnly),
            sync_mode: Some(SyncMode::LocalOnly),
            sensitivity: Some(Sensitivity::Secret),
        }
    }
}

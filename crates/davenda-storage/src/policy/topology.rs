use davenda_config::{
    ObjectStoreKind, PlatformConfig, SingleNodeStorageMode, StorageClass, StorageDeployment,
};

use super::StorageBackendKind;
use crate::policy::paths::trim_trailing_separator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreTarget {
    pub kind: ObjectStoreKind,
}

impl ObjectStoreTarget {
    pub const fn backend_kind(&self) -> StorageBackendKind {
        match self.kind {
            ObjectStoreKind::S3 => StorageBackendKind::S3Compatible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageTopology {
    pub local_root: String,
    pub default_class: StorageClass,
    pub deployment: StorageDeployment,
    pub single_node_escape_hatch: SingleNodeStorageMode,
    pub object_store: Option<ObjectStoreTarget>,
}

impl StorageTopology {
    pub fn from_config(config: &PlatformConfig) -> Self {
        Self {
            local_root: trim_trailing_separator(&config.storage.local_root),
            default_class: config.storage.default_class,
            deployment: config.storage.deployment,
            single_node_escape_hatch: config.storage.single_node_escape_hatch,
            object_store: config
                .storage
                .object_store
                .map(|kind| ObjectStoreTarget { kind }),
        }
    }

    pub fn supports_object_store(&self) -> bool {
        self.object_store.is_some()
    }

    pub const fn allows_explicit_local_only(&self) -> bool {
        matches!(self.deployment, StorageDeployment::SingleNode)
            && matches!(
                self.single_node_escape_hatch,
                SingleNodeStorageMode::ExplicitSingleNode
            )
    }
}

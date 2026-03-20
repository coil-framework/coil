use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::{StorageBackendKind, StoragePlan, StorageTopology, WriteTarget};

mod local;
mod config;
mod object_store;

#[cfg(test)]
mod tests;

use local::LocalDiskStorageClient;
pub use config::ObjectStoreClientConfig;
pub use object_store::HttpObjectStoreClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageDeliveryLocation {
    PublicCdn {
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
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageWriteReceipt {
    pub target: WriteTarget,
    pub path: PathBuf,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageReadReceipt {
    pub target: WriteTarget,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub bytes_read: u64,
}

pub trait ObjectStoreClient: fmt::Debug + Send + Sync {
    fn backend_kind(&self) -> StorageBackendKind;
    fn root(&self) -> &Path;
    fn is_configured(&self) -> bool;
    fn put(&self, object_key: &str, bytes: &[u8]) -> Result<PathBuf, StorageExecutionError>;
    fn get(&self, object_key: &str) -> Result<(PathBuf, Vec<u8>), StorageExecutionError>;
}

#[derive(Debug, Clone)]
pub struct StorageExecutor {
    local_client: LocalDiskStorageClient,
    object_store: Option<Arc<dyn ObjectStoreClient>>,
}

impl StorageExecutor {
    pub fn from_topology(topology: &StorageTopology) -> Self {
        Self::from_topology_and_object_store(topology, None)
    }

    pub fn from_topology_and_object_store(
        topology: &StorageTopology,
        object_store: Option<ObjectStoreClientConfig>,
    ) -> Self {
        let local_root = PathBuf::from(&topology.local_root);
        let object_store = topology.object_store.as_ref().map(|_| {
            Arc::new(HttpObjectStoreClient::from_topology_and_object_store(
                topology,
                object_store.clone(),
            )) as Arc<dyn ObjectStoreClient>
        });

        Self {
            local_client: LocalDiskStorageClient::new(local_root),
            object_store,
        }
    }

    pub fn execute_write(
        &self,
        plan: &StoragePlan,
        bytes: impl AsRef<[u8]>,
    ) -> Result<StorageWriteReceipt, StorageExecutionError> {
        let bytes = bytes.as_ref();
        let target = plan.primary_write_target().cloned().ok_or_else(|| {
            StorageExecutionError::MissingPrimaryWriteTarget {
                logical_path: plan.logical_path.clone(),
            }
        })?;

        let path = match target.backend {
            StorageBackendKind::LocalDisk => {
                let path = plan.local_path.as_ref().ok_or_else(|| {
                    StorageExecutionError::MissingLocalPath {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                self.local_client.write(Path::new(path), bytes)?
            }
            StorageBackendKind::S3Compatible => {
                let object_key = plan.object_key.as_deref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectKey {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                let client = self.object_store.as_ref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectStoreBackend {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                if !client.is_configured() {
                    return Err(StorageExecutionError::MissingObjectStoreEndpoint {
                        logical_path: plan.logical_path.clone(),
                    });
                }
                client.put(object_key, bytes)?
            }
        };

        Ok(StorageWriteReceipt {
            target,
            path,
            bytes_written: bytes.len() as u64,
        })
    }

    pub fn execute_read(
        &self,
        plan: &StoragePlan,
    ) -> Result<StorageReadReceipt, StorageExecutionError> {
        let target = plan.primary_write_target().cloned().ok_or_else(|| {
            StorageExecutionError::MissingPrimaryWriteTarget {
                logical_path: plan.logical_path.clone(),
            }
        })?;

        let (path, bytes): (PathBuf, Vec<u8>) = match target.backend {
            StorageBackendKind::LocalDisk => {
                let path = plan.local_path.as_ref().ok_or_else(|| {
                    StorageExecutionError::MissingLocalPath {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                self.local_client.read(Path::new(path))?
            }
            StorageBackendKind::S3Compatible => {
                let object_key = plan.object_key.as_deref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectKey {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                let client = self.object_store.as_ref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectStoreBackend {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                if !client.is_configured() {
                    return Err(StorageExecutionError::MissingObjectStoreEndpoint {
                        logical_path: plan.logical_path.clone(),
                    });
                }
                client.get(object_key)?
            }
        };

        Ok(StorageReadReceipt {
            target,
            path,
            bytes_read: bytes.len() as u64,
            bytes,
        })
    }

    pub fn delivery_location(
        &self,
        plan: &StoragePlan,
        cdn_base_url: Option<&str>,
    ) -> Result<StorageDeliveryLocation, StorageExecutionError> {
        match plan.policy.delivery_mode {
            crate::DeliveryMode::PublicCdn => {
                let object_key = plan.object_key.as_deref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectKey {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                let cdn_base_url =
                    cdn_base_url.ok_or_else(|| StorageExecutionError::MissingCdnBaseUrl {
                        logical_path: plan.logical_path.clone(),
                    })?;
                Ok(StorageDeliveryLocation::PublicCdn {
                    public_url: join_base_url(cdn_base_url, object_key),
                    object_key: object_key.to_string(),
                })
            }
            crate::DeliveryMode::SignedUrl => {
                let object_key = plan.object_key.as_deref().ok_or_else(|| {
                    StorageExecutionError::MissingObjectKey {
                        logical_path: plan.logical_path.clone(),
                    }
                })?;
                Ok(StorageDeliveryLocation::SignedObject {
                    object_key: object_key.to_string(),
                })
            }
            crate::DeliveryMode::AppProxy => Ok(StorageDeliveryLocation::AppProxy {
                path: plan.logical_path.clone(),
            }),
            crate::DeliveryMode::LocalOnly => {
                let path = plan
                    .local_path
                    .as_ref()
                    .ok_or_else(|| StorageExecutionError::MissingLocalPath {
                        logical_path: plan.logical_path.clone(),
                    })?
                    .clone();
                Ok(StorageDeliveryLocation::LocalPath {
                    path: PathBuf::from(path),
                })
            }
        }
    }
}

fn join_base_url(base_url: &str, object_key: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        object_key.trim_start_matches('/')
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageExecutionError {
    #[error("storage plan for `{logical_path}` does not provide a primary write target")]
    MissingPrimaryWriteTarget { logical_path: String },
    #[error("storage plan for `{logical_path}` requires a local path")]
    MissingLocalPath { logical_path: String },
    #[error("storage plan for `{logical_path}` requires an object key")]
    MissingObjectKey { logical_path: String },
    #[error("storage plan for `{logical_path}` requires an object-store backend")]
    MissingObjectStoreBackend { logical_path: String },
    #[error("storage plan for `{logical_path}` requires a configured object-store endpoint")]
    MissingObjectStoreEndpoint { logical_path: String },
    #[error("storage plan for `{logical_path}` requires `cdn_base_url` to resolve public delivery")]
    MissingCdnBaseUrl { logical_path: String },
    #[error("storage path `{path}` is outside the configured storage root")]
    InvalidTargetPath { path: String },
    #[error("failed to read storage path `{path}`: {message}")]
    ReadFailed { path: String, message: String },
    #[error("failed to write storage path `{path}`: {message}")]
    WriteFailed { path: String, message: String },
}

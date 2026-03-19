use std::path::{Path, PathBuf};

use super::{StorageBackendKind, StorageExecutionError};

#[derive(Debug, Clone)]
pub struct LocalFilesystemObjectStoreClient {
    root: PathBuf,
}

impl LocalFilesystemObjectStoreClient {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }
}

impl super::ObjectStoreClient for LocalFilesystemObjectStoreClient {
    fn backend_kind(&self) -> StorageBackendKind {
        StorageBackendKind::S3Compatible
    }

    fn root(&self) -> &Path {
        self.root()
    }

    fn put(&self, object_key: &str, bytes: &[u8]) -> Result<PathBuf, StorageExecutionError> {
        let path = self.resolve(object_key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StorageExecutionError::WriteFailed {
                    path: parent.display().to_string(),
                    message: error.to_string(),
                }
            })?;
        }
        std::fs::write(&path, bytes).map_err(|error| StorageExecutionError::WriteFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok(path)
    }

    fn get(&self, object_key: &str) -> Result<(PathBuf, Vec<u8>), StorageExecutionError> {
        let path = self.resolve(object_key)?;
        let bytes = std::fs::read(&path).map_err(|error| StorageExecutionError::ReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok((path, bytes))
    }
}

impl LocalFilesystemObjectStoreClient {
    fn resolve(&self, object_key: &str) -> Result<PathBuf, StorageExecutionError> {
        let normalized = normalize_object_key(object_key)?;
        let resolved = self.root.join(&normalized);
        if !resolved.starts_with(&self.root) {
            return Err(StorageExecutionError::InvalidTargetPath {
                path: resolved.display().to_string(),
            });
        }
        Ok(resolved)
    }
}

fn normalize_object_key(object_key: &str) -> Result<String, StorageExecutionError> {
    let mut parts = Vec::new();
    for component in Path::new(object_key).components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => {
                return Err(StorageExecutionError::InvalidTargetPath {
                    path: object_key.to_string(),
                });
            }
        }
    }

    if parts.is_empty() {
        return Err(StorageExecutionError::InvalidTargetPath {
            path: object_key.to_string(),
        });
    }

    Ok(parts.join("/"))
}

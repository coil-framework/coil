use std::path::{Path, PathBuf};

use super::StorageExecutionError;

#[derive(Debug, Clone)]
pub(crate) struct LocalDiskStorageClient {
    root: PathBuf,
}

impl LocalDiskStorageClient {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(super) fn write(
        &self,
        path: &Path,
        bytes: &[u8],
    ) -> Result<PathBuf, StorageExecutionError> {
        let path = self.resolve(path)?;
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

    pub(super) fn read(&self, path: &Path) -> Result<(PathBuf, Vec<u8>), StorageExecutionError> {
        let path = self.resolve(path)?;
        let bytes = std::fs::read(&path).map_err(|error| StorageExecutionError::ReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok((path, bytes))
    }

    fn resolve(&self, path: &Path) -> Result<PathBuf, StorageExecutionError> {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };

        if !resolved.starts_with(&self.root) {
            return Err(StorageExecutionError::InvalidTargetPath {
                path: resolved.display().to_string(),
            });
        }

        Ok(resolved)
    }
}

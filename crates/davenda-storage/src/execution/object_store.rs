use std::io::Read;
use std::path::{Path, PathBuf};

use ureq::AgentBuilder;
use url::Url;

use super::{StorageBackendKind, StorageExecutionError};

#[derive(Debug, Clone)]
pub struct HttpObjectStoreClient {
    endpoint: Option<Url>,
    root: PathBuf,
}

impl HttpObjectStoreClient {
    pub fn from_topology(_topology: &crate::StorageTopology) -> Self {
        let endpoint = std::env::var("OBJECT_STORE_URL")
            .ok()
            .and_then(|value| Url::parse(value.trim()).ok());
        let root = endpoint
            .as_ref()
            .and_then(|url| {
                if url.scheme() == "file" {
                    url.to_file_path().ok()
                } else {
                    Some(PathBuf::from(url.path().trim_start_matches('/')))
                }
            })
            .unwrap_or_default();

        Self { endpoint, root }
    }

    fn resolve(&self, object_key: &str) -> Result<Url, StorageExecutionError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| StorageExecutionError::MissingObjectStoreEndpoint {
                logical_path: object_key.to_string(),
            })?;
        let mut url = endpoint.clone();
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| StorageExecutionError::InvalidTargetPath {
                    path: object_key.to_string(),
                })?;
            for segment in normalize_object_key(object_key)?.split('/') {
                segments.push(segment);
            }
        }
        Ok(url)
    }

    fn object_path(&self, object_key: &str) -> Result<PathBuf, StorageExecutionError> {
        Ok(self.root.join(normalize_object_key(object_key)?))
    }
}

impl super::ObjectStoreClient for HttpObjectStoreClient {
    fn backend_kind(&self) -> StorageBackendKind {
        StorageBackendKind::S3Compatible
    }

    fn root(&self) -> &Path {
        self.root.as_path()
    }

    fn is_configured(&self) -> bool {
        self.endpoint.is_some()
    }

    fn put(&self, object_key: &str, bytes: &[u8]) -> Result<PathBuf, StorageExecutionError> {
        if self
            .endpoint
            .as_ref()
            .is_some_and(|endpoint| endpoint.scheme() == "file")
        {
            let path = self.object_path(object_key)?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| StorageExecutionError::WriteFailed {
                    path: parent.display().to_string(),
                    message: error.to_string(),
                })?;
            }
            std::fs::write(&path, bytes).map_err(|error| StorageExecutionError::WriteFailed {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            return Ok(path);
        }

        let url = self.resolve(object_key)?;
        let agent = AgentBuilder::new().build();
        let response = agent
            .put(url.as_str())
            .set("Content-Type", "application/octet-stream")
            .send_bytes(bytes)
            .map_err(|error| StorageExecutionError::WriteFailed {
                path: object_key.to_string(),
                message: error.to_string(),
            })?;

        if !(200..300).contains(&response.status()) {
            return Err(StorageExecutionError::WriteFailed {
                path: object_key.to_string(),
                message: format!("unexpected status {}", response.status()),
            });
        }

        Ok(PathBuf::from(normalize_object_key(object_key)?))
    }

    fn get(&self, object_key: &str) -> Result<(PathBuf, Vec<u8>), StorageExecutionError> {
        if self
            .endpoint
            .as_ref()
            .is_some_and(|endpoint| endpoint.scheme() == "file")
        {
            let path = self.object_path(object_key)?;
            let bytes = std::fs::read(&path).map_err(|error| StorageExecutionError::ReadFailed {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            return Ok((path, bytes));
        }

        let url = self.resolve(object_key)?;
        let agent = AgentBuilder::new().build();
        let response = agent
            .get(url.as_str())
            .call()
            .map_err(|error| StorageExecutionError::ReadFailed {
                path: object_key.to_string(),
                message: error.to_string(),
            })?;

        if !(200..300).contains(&response.status()) {
            return Err(StorageExecutionError::ReadFailed {
                path: object_key.to_string(),
                message: format!("unexpected status {}", response.status()),
            });
        }

        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|error| StorageExecutionError::ReadFailed {
                path: object_key.to_string(),
                message: error.to_string(),
            })?;
        Ok((PathBuf::from(normalize_object_key(object_key)?), bytes))
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

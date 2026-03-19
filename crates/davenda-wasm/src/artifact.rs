use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::WasmModelError;
use crate::ids::ExtensionId;
use crate::manifest::ExtensionArtifactSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArtifact {
    pub publisher: String,
    pub source: ExtensionArtifactSource,
    pub sha256: String,
}

impl InstalledArtifact {
    pub fn new(
        publisher: impl Into<String>,
        source: ExtensionArtifactSource,
        sha256: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            publisher: crate::validation::require_non_empty(
                "extension_publisher",
                publisher.into(),
            )?,
            source,
            sha256: crate::validation::validate_sha256("extension_artifact_sha256", sha256.into())?,
        })
    }

    pub fn resolve_path(&self, base_dir: impl AsRef<Path>, extension_id: &ExtensionId) -> PathBuf {
        self.source.resolve_path(base_dir, extension_id)
    }

    pub fn load_bytes(
        &self,
        base_dir: impl AsRef<Path>,
        extension_id: &ExtensionId,
    ) -> Result<Vec<u8>, WasmModelError> {
        let path = self.resolve_path(base_dir, extension_id);
        let bytes = fs::read(&path).map_err(|error| WasmModelError::ArtifactRead {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        verify_checksum(&self.sha256, &path, &bytes)?;
        Ok(bytes)
    }
}

impl ExtensionArtifactSource {
    pub fn resolve_path(&self, base_dir: impl AsRef<Path>, _extension_id: &ExtensionId) -> PathBuf {
        let base_dir = base_dir.as_ref();
        match self {
            Self::LocalPath(path) => {
                let path = PathBuf::from(path);
                if path.is_absolute() {
                    path
                } else {
                    base_dir.join(path)
                }
            }
            Self::RegistryPackage { registry, package } => base_dir
                .join("registry")
                .join(registry)
                .join(format!("{package}.wasm")),
            Self::FirstPartyCatalog { package } => {
                base_dir.join("catalog").join(format!("{package}.wasm"))
            }
        }
    }
}

fn verify_checksum(expected: &str, path: &Path, bytes: &[u8]) -> Result<(), WasmModelError> {
    let actual = hex::encode(Sha256::digest(bytes));
    if actual == expected {
        Ok(())
    } else {
        Err(WasmModelError::ArtifactChecksumMismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

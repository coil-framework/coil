use super::*;
use std::collections::{BTreeMap, btree_map::Entry};

use coil_storage::{StoragePlanRequest, StoragePlanner, StoragePolicy};

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
                .plan_scalable_write(
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

use davenda_storage::{DeliveryMode, StoragePlanningError, StoragePolicy};
use thiserror::Error;

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

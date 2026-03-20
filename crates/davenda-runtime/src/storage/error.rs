use thiserror::Error;

use davenda_assets::AssetModelError;
use davenda_storage::{StoragePlanningError, execution::StorageExecutionError};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeStorageError {
    #[error(transparent)]
    Storage(#[from] StoragePlanningError),
    #[error(transparent)]
    Execution(#[from] StorageExecutionError),
    #[error(transparent)]
    Asset(#[from] AssetModelError),
    #[error("assets.cdn_base_url must be configured for public asset publication")]
    MissingCdnBaseUrl,
    #[error("asset publication authorization failed for `{asset_id}`: {reason}")]
    PublicationAuthorizationDenied { asset_id: String, reason: String },
}

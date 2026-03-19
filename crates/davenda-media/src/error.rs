use crate::model::PublicationStatus;
use davenda_storage::StoragePolicyError;
use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum MediaModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateIdentifier {
        kind: &'static str,
        id: String,
    },
    MissingLibrary {
        library_id: String,
    },
    MissingFolder {
        folder_id: String,
    },
    MissingAsset {
        asset_id: String,
    },
    MissingLiveRevision {
        asset_id: String,
    },
    MissingStagedReplacement {
        asset_id: String,
    },
    InvalidRevisionTransition {
        asset_id: String,
        from: PublicationStatus,
        to: PublicationStatus,
    },
    StoragePolicy {
        error: StoragePolicyError,
    },
}

impl fmt::Display for MediaModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => {
                write!(f, "{kind} `{id}` is duplicated")
            }
            Self::MissingLibrary { library_id } => {
                write!(f, "media library `{library_id}` was not found")
            }
            Self::MissingFolder { folder_id } => {
                write!(f, "media folder `{folder_id}` was not found")
            }
            Self::MissingAsset { asset_id } => {
                write!(f, "media asset `{asset_id}` was not found")
            }
            Self::MissingLiveRevision { asset_id } => {
                write!(f, "media asset `{asset_id}` has no live revision")
            }
            Self::MissingStagedReplacement { asset_id } => {
                write!(f, "media asset `{asset_id}` has no staged replacement")
            }
            Self::InvalidRevisionTransition { asset_id, from, to } => write!(
                f,
                "media asset `{asset_id}` cannot transition from `{from}` to `{to}`"
            ),
            Self::StoragePolicy { error } => write!(f, "{error}"),
        }
    }
}

impl Error for MediaModelError {}

impl From<StoragePolicyError> for MediaModelError {
    fn from(error: StoragePolicyError) -> Self {
        Self::StoragePolicy { error }
    }
}

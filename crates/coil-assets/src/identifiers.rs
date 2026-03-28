use std::fmt;

use crate::{AssetModelError, require_non_empty};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(String);

impl AssetId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("asset_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("revision_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseId(String);

impl ReleaseId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetModelError> {
        Ok(Self(require_non_empty("release_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReleaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

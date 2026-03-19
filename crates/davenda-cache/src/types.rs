use std::fmt;

use crate::CacheModelError;

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, CacheModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CacheModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, CacheModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CacheModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariationKey(String);

impl VariationKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VariationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InvalidationTag(String);

impl InvalidationTag {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("invalidation_tag", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvalidationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheNamespace(String);

impl CacheNamespace {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("cache_namespace", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CacheNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheKey {
    namespace: CacheNamespace,
    resource: String,
    variation: Option<VariationKey>,
}

impl CacheKey {
    pub fn new(
        namespace: CacheNamespace,
        resource: impl Into<String>,
        variation: Option<VariationKey>,
    ) -> Result<Self, CacheModelError> {
        Ok(Self {
            namespace,
            resource: require_non_empty("cache_resource", resource.into())?,
            variation,
        })
    }

    pub fn namespace(&self) -> &CacheNamespace {
        &self.namespace
    }

    pub fn resource(&self) -> &str {
        &self.resource
    }

    pub fn variation(&self) -> Option<&VariationKey> {
        self.variation.as_ref()
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variation {
            Some(variation) => write!(f, "{}:{}|{}", self.namespace, self.resource, variation),
            None => write!(f, "{}:{}", self.namespace, self.resource),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityTag(String);

impl EntityTag {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("etag", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheInstant(u64);

impl CacheInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }
}

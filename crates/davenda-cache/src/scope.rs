use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;

use crate::types::validate_token;
use crate::{CacheModelError, VariationKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheVisibility {
    Public,
    Private,
    NoStore,
}

impl fmt::Display for CacheVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Private => f.write_str("private"),
            Self::NoStore => f.write_str("no_store"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheScope {
    visibility: CacheVisibility,
    tenant: Option<String>,
    site: Option<String>,
    locale: Option<String>,
    user: Option<String>,
    session: Option<String>,
    custom: BTreeMap<String, String>,
}

impl CacheScope {
    pub fn public() -> Self {
        Self::new(CacheVisibility::Public)
    }

    pub fn private() -> Self {
        Self::new(CacheVisibility::Private)
    }

    pub fn no_store() -> Self {
        Self::new(CacheVisibility::NoStore)
    }

    pub fn new(visibility: CacheVisibility) -> Self {
        Self {
            visibility,
            tenant: None,
            site: None,
            locale: None,
            user: None,
            session: None,
            custom: BTreeMap::new(),
        }
    }

    pub fn visibility(&self) -> CacheVisibility {
        self.visibility
    }

    pub fn tenant(&self) -> Option<&str> {
        self.tenant.as_deref()
    }

    pub fn site(&self) -> Option<&str> {
        self.site.as_deref()
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub fn custom(&self) -> &BTreeMap<String, String> {
        &self.custom
    }

    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Result<Self, CacheModelError> {
        self.tenant = Some(validate_token("tenant", tenant.into())?);
        Ok(self)
    }

    pub fn with_site(mut self, site: impl Into<String>) -> Result<Self, CacheModelError> {
        self.site = Some(validate_token("site", site.into())?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, CacheModelError> {
        self.locale = Some(validate_token("locale", locale.into())?);
        Ok(self)
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Result<Self, CacheModelError> {
        if self.visibility == CacheVisibility::Public {
            return Err(CacheModelError::PublicScopeCannotVaryByUser);
        }

        self.user = Some(validate_token("user", user.into())?);
        Ok(self)
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Result<Self, CacheModelError> {
        if self.visibility == CacheVisibility::Public {
            return Err(CacheModelError::PublicScopeCannotVaryBySession);
        }

        self.session = Some(validate_token("session", session.into())?);
        Ok(self)
    }

    pub fn with_custom_variation(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, CacheModelError> {
        let name = validate_token("variation_name", name.into())?;
        let value = validate_token("variation_value", value.into())?;
        self.custom.insert(name, value);
        Ok(self)
    }

    pub fn is_cacheable(&self) -> bool {
        self.visibility != CacheVisibility::NoStore
    }

    pub fn is_edge_cacheable(&self) -> bool {
        self.visibility == CacheVisibility::Public
    }

    pub fn variation_key(&self) -> Option<VariationKey> {
        self.variation_key_internal(false)
    }

    pub fn cache_partition_key(&self) -> Option<VariationKey> {
        self.variation_key_internal(true)
    }

    fn variation_key_internal(&self, include_visibility: bool) -> Option<VariationKey> {
        if self.visibility == CacheVisibility::NoStore {
            return None;
        }

        let mut parts = Vec::new();
        if include_visibility {
            parts.push(format!("visibility={}", self.visibility));
        }

        if let Some(tenant) = &self.tenant {
            parts.push(format!("tenant={tenant}"));
        }

        if let Some(site) = &self.site {
            parts.push(format!("site={site}"));
        }

        if let Some(locale) = &self.locale {
            parts.push(format!("locale={locale}"));
        }

        if let Some(user) = &self.user {
            parts.push(format!("user={user}"));
        }

        if let Some(session) = &self.session {
            parts.push(format!("session={session}"));
        }

        for (name, value) in &self.custom {
            parts.push(format!("x:{name}={value}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(VariationKey::new(parts.join("|")))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvalidationSet {
    tags: BTreeSet<crate::InvalidationTag>,
}

impl InvalidationSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_tags(tags: impl IntoIterator<Item = crate::InvalidationTag>) -> Self {
        let mut set = Self::new();
        for tag in tags {
            set.insert(tag);
        }
        set
    }

    pub fn insert(&mut self, tag: crate::InvalidationTag) {
        self.tags.insert(tag);
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &crate::InvalidationTag> {
        self.tags.iter()
    }

    pub fn header_value(&self) -> Option<String> {
        if self.tags.is_empty() {
            None
        } else {
            Some(
                self.tags
                    .iter()
                    .map(crate::InvalidationTag::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessPolicy {
    ttl: Duration,
    stale_while_revalidate: Option<Duration>,
}

impl FreshnessPolicy {
    pub fn new(
        ttl: Duration,
        stale_while_revalidate: Option<Duration>,
    ) -> Result<Self, CacheModelError> {
        if ttl.is_zero() {
            return Err(CacheModelError::ZeroDuration { field: "ttl" });
        }

        if stale_while_revalidate.is_some_and(|value| value.is_zero()) {
            return Err(CacheModelError::ZeroDuration {
                field: "stale_while_revalidate",
            });
        }

        Ok(Self {
            ttl,
            stale_while_revalidate,
        })
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn stale_while_revalidate(&self) -> Option<Duration> {
        self.stale_while_revalidate
    }

    pub fn ttl_seconds(&self) -> u64 {
        self.ttl.as_secs()
    }

    pub fn stale_while_revalidate_seconds(&self) -> Option<u64> {
        self.stale_while_revalidate.map(|value| value.as_secs())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResponseValidators {
    pub etag: Option<crate::EntityTag>,
    pub last_modified_unix_seconds: Option<u64>,
}

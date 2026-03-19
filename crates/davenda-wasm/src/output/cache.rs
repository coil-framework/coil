use std::collections::BTreeSet;

use crate::error::WasmModelError;
use crate::validation::validate_token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCacheHint {
    pub visibility: CacheVisibility,
    pub max_age_seconds: u64,
    pub stale_while_revalidate_seconds: Option<u64>,
    pub vary_by_locale: bool,
    pub vary_by_user: bool,
    pub vary_by_session: bool,
    pub tags: BTreeSet<String>,
}

impl TypedCacheHint {
    pub fn new(
        visibility: CacheVisibility,
        max_age_seconds: u64,
        stale_while_revalidate_seconds: Option<u64>,
        vary_by_locale: bool,
        vary_by_user: bool,
        vary_by_session: bool,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, WasmModelError> {
        let tags = tags
            .into_iter()
            .map(|tag| validate_token("cache_tag", tag.into()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let hint = Self {
            visibility,
            max_age_seconds,
            stale_while_revalidate_seconds,
            vary_by_locale,
            vary_by_user,
            vary_by_session,
            tags,
        };
        hint.validate()?;
        Ok(hint)
    }

    pub(crate) fn validate(&self) -> Result<(), WasmModelError> {
        if self.visibility == CacheVisibility::Public && (self.vary_by_user || self.vary_by_session)
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "public cache hints cannot vary by user or session".to_string(),
            });
        }
        if self
            .stale_while_revalidate_seconds
            .is_some_and(|value| value == 0)
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "stale-while-revalidate must be greater than zero".to_string(),
            });
        }
        for tag in &self.tags {
            let _ = validate_token("cache_tag", tag.clone())?;
        }
        Ok(())
    }

    pub fn merge_from(&mut self, other: &Self) {
        self.visibility = match (self.visibility, other.visibility) {
            (CacheVisibility::Private, _) | (_, CacheVisibility::Private) => {
                CacheVisibility::Private
            }
            _ => CacheVisibility::Public,
        };
        self.max_age_seconds = self.max_age_seconds.min(other.max_age_seconds);
        self.stale_while_revalidate_seconds = match (
            self.stale_while_revalidate_seconds,
            other.stale_while_revalidate_seconds,
        ) {
            (Some(left), Some(right)) => Some(left.min(right)),
            _ => None,
        };
        self.vary_by_locale |= other.vary_by_locale;
        self.vary_by_user |= other.vary_by_user;
        self.vary_by_session |= other.vary_by_session;
        self.tags.extend(other.tags.iter().cloned());
    }
}

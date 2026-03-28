use crate::{CacheModelError, CacheScope, FreshnessPolicy, InvalidationSet, ResponseValidators};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCachePolicy {
    scope: CacheScope,
    freshness: FreshnessPolicy,
    tags: InvalidationSet,
}

impl ApplicationCachePolicy {
    pub fn new(
        scope: CacheScope,
        freshness: FreshnessPolicy,
        tags: InvalidationSet,
    ) -> Result<Self, CacheModelError> {
        if !scope.is_cacheable() {
            return Err(CacheModelError::UncacheableApplicationScope);
        }

        Ok(Self {
            scope,
            freshness,
            tags,
        })
    }

    pub fn scope(&self) -> &CacheScope {
        &self.scope
    }

    pub fn freshness(&self) -> FreshnessPolicy {
        self.freshness
    }

    pub fn tags(&self) -> &InvalidationSet {
        &self.tags
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCachePolicy {
    scope: CacheScope,
    freshness: Option<FreshnessPolicy>,
    validators: ResponseValidators,
    surrogate_tags: InvalidationSet,
}

impl HttpCachePolicy {
    pub fn new(
        scope: CacheScope,
        freshness: Option<FreshnessPolicy>,
        validators: ResponseValidators,
        surrogate_tags: InvalidationSet,
    ) -> Result<Self, CacheModelError> {
        match (scope.is_cacheable(), freshness) {
            (true, None) => Err(CacheModelError::MissingHttpFreshness),
            (false, Some(_)) => Err(CacheModelError::NoStoreCannotDefineFreshness),
            _ => Ok(Self {
                scope,
                freshness,
                validators,
                surrogate_tags,
            }),
        }
    }

    pub fn scope(&self) -> &CacheScope {
        &self.scope
    }

    pub fn freshness(&self) -> Option<FreshnessPolicy> {
        self.freshness
    }

    pub fn validators(&self) -> &ResponseValidators {
        &self.validators
    }

    pub fn surrogate_tags(&self) -> &InvalidationSet {
        &self.surrogate_tags
    }

    pub fn cache_control_value(&self) -> String {
        match (self.scope.visibility(), self.freshness) {
            (crate::CacheVisibility::NoStore, _) => "no-store".to_string(),
            (crate::CacheVisibility::Public, Some(freshness)) => {
                cache_control_value("public", freshness)
            }
            (crate::CacheVisibility::Private, Some(freshness)) => {
                cache_control_value("private", freshness)
            }
            (_, None) => "no-store".to_string(),
        }
    }
}

fn cache_control_value(visibility: &str, freshness: FreshnessPolicy) -> String {
    let mut directives = vec![format!("{visibility}, max-age={}", freshness.ttl_seconds())];

    if let Some(swr) = freshness.stale_while_revalidate_seconds() {
        directives.push(format!("stale-while-revalidate={swr}"));
    }

    directives.join(", ")
}

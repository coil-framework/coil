use crate::{
    CacheKey, CacheLayerPlan, CacheModelError, CacheTopology, FreshnessPolicy, InvalidationSet,
    RequestCoalescingMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub value: String,
    pub stored_at: crate::CacheInstant,
    pub freshness: FreshnessPolicy,
    pub tags: InvalidationSet,
    pub scope: crate::CacheScope,
    pub layers: CacheLayerPlan,
}

impl CacheEntry {
    pub fn age_seconds(&self, now: crate::CacheInstant) -> u64 {
        now.as_unix_seconds()
            .saturating_sub(self.stored_at.as_unix_seconds())
    }

    pub fn is_fresh(&self, now: crate::CacheInstant) -> bool {
        self.age_seconds(now) <= self.freshness.ttl_seconds()
    }

    pub fn is_stale_but_servable(&self, now: crate::CacheInstant) -> bool {
        if self.is_fresh(now) {
            return false;
        }

        self.freshness
            .stale_while_revalidate_seconds()
            .is_some_and(|swr| {
                self.age_seconds(now) <= self.freshness.ttl_seconds().saturating_add(swr)
            })
    }

    pub fn is_expired(&self, now: crate::CacheInstant) -> bool {
        !self.is_fresh(now) && !self.is_stale_but_servable(now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLookupState {
    Miss,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLookup {
    pub state: CacheLookupState,
    pub entry: Option<CacheEntry>,
    pub needs_revalidation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetrics {
    pub hits: u64,
    pub stale_hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub coalesced_waits: u64,
    pub fills_started: u64,
    pub fills_completed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillDecision {
    Start(FillLease),
    Coalesced { key: CacheKey, holder: String },
    Uncoalesced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillLease {
    pub key: CacheKey,
    pub holder: String,
}

#[derive(Debug, Clone)]
pub struct CacheRuntime {
    topology: CacheTopology,
    backend: crate::CacheBackendAdapter,
}

impl CacheRuntime {
    pub fn new(topology: CacheTopology) -> Self {
        Self {
            topology,
            backend: crate::CacheBackendAdapter::new(topology),
        }
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn backend_kind(&self) -> crate::CacheBackendKind {
        self.backend.kind()
    }

    pub fn backend_is_shared(&self) -> bool {
        self.backend.is_shared()
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.backend.metrics()
    }

    pub fn insert(
        &mut self,
        plan: &crate::ApplicationCachePlan,
        value: impl Into<String>,
        now: crate::CacheInstant,
    ) {
        let entry = CacheEntry {
            key: plan.key().clone(),
            value: value.into(),
            stored_at: now,
            freshness: plan.freshness(),
            tags: plan.tags().clone(),
            scope: plan.scope().clone(),
            layers: plan.layers().clone(),
        };
        self.backend.insert(entry);
    }

    pub fn lookup(&mut self, key: &CacheKey, now: crate::CacheInstant) -> CacheLookup {
        self.backend.lookup(key, now)
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.backend.invalidate(tags)
    }

    pub fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        self.backend.begin_fill(key, mode, holder)
    }

    pub fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.backend.complete_fill(lease)
    }
}

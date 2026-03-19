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
    repository: crate::repository::CacheRepository,
    metrics: CacheMetrics,
}

impl CacheRuntime {
    pub fn new(topology: CacheTopology) -> Self {
        Self {
            topology,
            repository: crate::repository::CacheRepository::new(),
            metrics: CacheMetrics::default(),
        }
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.metrics
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
        self.repository.insert(entry);
    }

    pub fn lookup(&mut self, key: &CacheKey, now: crate::CacheInstant) -> CacheLookup {
        let Some(entry) = self.repository.lookup(key) else {
            self.metrics.misses += 1;
            return CacheLookup {
                state: CacheLookupState::Miss,
                entry: None,
                needs_revalidation: false,
            };
        };

        if entry.is_fresh(now) {
            self.metrics.hits += 1;
            return CacheLookup {
                state: CacheLookupState::Fresh,
                entry: Some(entry),
                needs_revalidation: false,
            };
        }

        if entry.is_stale_but_servable(now) {
            self.metrics.stale_hits += 1;
            return CacheLookup {
                state: CacheLookupState::Stale,
                entry: Some(entry),
                needs_revalidation: true,
            };
        }

        let _ = self.repository.remove(key);
        self.metrics.misses += 1;
        CacheLookup {
            state: CacheLookupState::Miss,
            entry: None,
            needs_revalidation: false,
        }
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        let removed = self.repository.invalidate(tags);
        self.metrics.invalidations += removed.len() as u64;
        removed
    }

    pub fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        let decision = self.repository.begin_fill(key, mode, holder);
        match &decision {
            FillDecision::Start(_) => {
                self.metrics.fills_started += 1;
            }
            FillDecision::Coalesced { .. } => {
                self.metrics.coalesced_waits += 1;
            }
            FillDecision::Uncoalesced => {}
        }
        decision
    }

    pub fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.repository.complete_fill(lease)?;
        self.metrics.fills_completed += 1;
        Ok(())
    }
}

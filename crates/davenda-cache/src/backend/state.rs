use crate::{
    CacheEntry, CacheKey, CacheLookup, CacheLookupState, CacheMetrics, CacheModelError,
    FillDecision, FillLease, InvalidationSet, RequestCoalescingMode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CacheBackendState {
    repository: crate::repository::CacheRepository,
    metrics: CacheMetrics,
}

impl CacheBackendState {
    pub(crate) fn new() -> Self {
        Self {
            repository: crate::repository::CacheRepository::new(),
            metrics: CacheMetrics::default(),
        }
    }

    pub(crate) fn insert(&mut self, entry: CacheEntry) {
        self.repository.insert(entry);
    }

    pub(crate) fn lookup(&mut self, key: &CacheKey, now: crate::CacheInstant) -> CacheLookup {
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

    pub(crate) fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        let removed = self.repository.invalidate(tags);
        self.metrics.invalidations += removed.len() as u64;
        removed
    }

    pub(crate) fn begin_fill(
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

    pub(crate) fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.repository.complete_fill(lease)?;
        self.metrics.fills_completed += 1;
        Ok(())
    }

    pub(crate) fn metrics(&self) -> CacheMetrics {
        self.metrics
    }
}

use crate::{
    CacheEntry, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision, FillLease,
    InvalidationSet, RequestCoalescingMode,
};

use super::state::CacheBackendState;

#[derive(Debug, Clone)]
pub(super) struct LocalCacheBackendAdapter {
    state: CacheBackendState,
}

impl LocalCacheBackendAdapter {
    pub(super) fn new() -> Self {
        Self {
            state: CacheBackendState::new(),
        }
    }

    pub(super) fn insert(&mut self, entry: CacheEntry) {
        self.state.insert(entry);
    }

    pub(super) fn lookup(&mut self, key: &CacheKey, now: crate::CacheInstant) -> CacheLookup {
        self.state.lookup(key, now)
    }

    pub(super) fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.state.invalidate(tags)
    }

    pub(super) fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        self.state.begin_fill(key, mode, holder)
    }

    pub(super) fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.state.complete_fill(lease)
    }

    pub(super) fn metrics(&self) -> CacheMetrics {
        self.state.metrics()
    }
}

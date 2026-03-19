use std::sync::{Arc, Mutex};

use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheLookupState, CacheMetrics,
    CacheModelError, CacheTopology, FillDecision, FillLease, InvalidationSet,
    RequestCoalescingMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheBackendKind {
    Local,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Default)]
struct CacheBackendState {
    repository: crate::repository::CacheRepository,
    metrics: CacheMetrics,
}

impl CacheBackendState {
    fn new() -> Self {
        Self {
            repository: crate::repository::CacheRepository::new(),
            metrics: CacheMetrics::default(),
        }
    }

    fn insert(&mut self, entry: CacheEntry) {
        self.repository.insert(entry);
    }

    fn lookup(&mut self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
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

    fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        let removed = self.repository.invalidate(tags);
        self.metrics.invalidations += removed.len() as u64;
        removed
    }

    fn begin_fill(
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

    fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.repository.complete_fill(lease)?;
        self.metrics.fills_completed += 1;
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum CacheBackendStorage {
    Local(CacheBackendState),
    Shared(Arc<Mutex<CacheBackendState>>),
}

impl CacheBackendStorage {
    fn local() -> Self {
        Self::Local(CacheBackendState::new())
    }

    fn shared() -> Self {
        Self::Shared(Arc::new(Mutex::new(CacheBackendState::new())))
    }

    fn with_state<R>(&self, f: impl FnOnce(&CacheBackendState) -> R) -> R {
        match self {
            Self::Local(state) => f(state),
            Self::Shared(state) => {
                let guard = state.lock().expect("cache backend mutex poisoned");
                f(&guard)
            }
        }
    }

    fn with_state_mut<R>(&mut self, f: impl FnOnce(&mut CacheBackendState) -> R) -> R {
        match self {
            Self::Local(state) => f(state),
            Self::Shared(state) => {
                let mut guard = state.lock().expect("cache backend mutex poisoned");
                f(&mut guard)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheBackendAdapter {
    kind: CacheBackendKind,
    topology: CacheTopology,
    storage: CacheBackendStorage,
}

impl CacheBackendAdapter {
    pub fn new(topology: CacheTopology) -> Self {
        let kind = match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
            Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
            None => CacheBackendKind::Local,
        };
        let storage = if topology.supports_shared_invalidation() {
            CacheBackendStorage::shared()
        } else {
            CacheBackendStorage::local()
        };

        Self {
            kind,
            topology,
            storage,
        }
    }

    pub fn kind(&self) -> CacheBackendKind {
        self.kind
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn is_shared(&self) -> bool {
        matches!(self.storage, CacheBackendStorage::Shared(_))
    }

    pub fn insert(&mut self, entry: CacheEntry) {
        self.storage.with_state_mut(|state| state.insert(entry));
    }

    pub fn lookup(&mut self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.storage.with_state_mut(|state| state.lookup(key, now))
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.storage.with_state_mut(|state| state.invalidate(tags))
    }

    pub fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        self.storage
            .with_state_mut(|state| state.begin_fill(key, mode, holder))
    }

    pub fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.storage
            .with_state_mut(|state| state.complete_fill(lease))
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.storage.with_state(|state| state.metrics)
    }
}

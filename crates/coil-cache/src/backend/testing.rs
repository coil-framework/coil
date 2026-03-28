use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision,
    FillLease, InvalidationSet, RequestCoalescingMode,
};

use super::state::CacheBackendState;
use super::{CacheBackendKind, DistributedCacheRuntime};

#[derive(Debug)]
struct EmulatedDistributedCacheRuntime {
    state: Mutex<CacheBackendState>,
}

impl EmulatedDistributedCacheRuntime {
    fn new() -> Self {
        Self {
            state: Mutex::new(CacheBackendState::new()),
        }
    }
}

impl DistributedCacheRuntime for EmulatedDistributedCacheRuntime {
    fn insert(&self, entry: CacheEntry) {
        let mut guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.insert(entry);
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        let mut guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.lookup(key, now)
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        let mut guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        let mut guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.begin_fill(key, mode, holder)
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        let mut guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.complete_fill(lease)
    }

    fn metrics(&self) -> CacheMetrics {
        let guard = self.state.lock().expect("cache backend mutex poisoned");
        guard.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        false
    }
}

pub(crate) fn test_scope_namespace() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test")
        .to_string()
}

pub(crate) fn test_only_sqlite_shared_runtime(
    kind: CacheBackendKind,
    namespace: String,
) -> Arc<dyn DistributedCacheRuntime> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, Arc<dyn DistributedCacheRuntime>>>> =
        OnceLock::new();

    let key = format!("{}:{kind:?}:{namespace}", test_scope_namespace());
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(EmulatedDistributedCacheRuntime::new()))
        .clone()
}

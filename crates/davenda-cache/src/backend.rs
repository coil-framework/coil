use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

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

pub trait DistributedCacheRuntime: Send + Sync + 'static {
    fn insert(&self, entry: CacheEntry);
    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup;
    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey>;
    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision;
    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError>;
    fn metrics(&self) -> CacheMetrics;
}

#[derive(Debug)]
struct SharedDistributedCacheRuntime {
    state: Mutex<CacheBackendState>,
}

impl SharedDistributedCacheRuntime {
    fn new() -> Self {
        Self {
            state: Mutex::new(CacheBackendState::new()),
        }
    }
}

impl DistributedCacheRuntime for SharedDistributedCacheRuntime {
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
        guard.metrics
    }
}

#[derive(Clone)]
pub struct DistributedCacheClient {
    kind: CacheBackendKind,
    runtime: Arc<dyn DistributedCacheRuntime>,
}

impl DistributedCacheClient {
    pub fn new(kind: CacheBackendKind, runtime: Arc<dyn DistributedCacheRuntime>) -> Self {
        Self { kind, runtime }
    }

    pub fn in_memory(kind: CacheBackendKind) -> Self {
        Self::new(kind, Arc::new(SharedDistributedCacheRuntime::new()))
    }

    pub fn shared(kind: CacheBackendKind) -> Self {
        Self::scoped_shared(kind, "global")
    }

    pub fn scoped_shared(kind: CacheBackendKind, scope: impl Into<String>) -> Self {
        Self::new(kind, shared_cache_runtime(kind, scope.into()))
    }

    pub fn kind(&self) -> CacheBackendKind {
        self.kind
    }

    pub fn is_shared(&self) -> bool {
        Arc::strong_count(&self.runtime) > 1
    }

    pub fn insert(&self, entry: CacheEntry) {
        self.runtime.insert(entry);
    }

    pub fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.runtime.lookup(key, now)
    }

    pub fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.runtime.invalidate(tags)
    }

    pub fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        self.runtime.begin_fill(key, mode, holder.into())
    }

    pub fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.runtime.metrics()
    }
}

impl fmt::Debug for DistributedCacheClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DistributedCacheClient")
            .field("kind", &self.kind)
            .finish()
    }
}

fn shared_cache_runtime(kind: CacheBackendKind, scope: String) -> Arc<dyn DistributedCacheRuntime> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, Arc<dyn DistributedCacheRuntime>>>> =
        OnceLock::new();

    let key = format!("{kind:?}:{scope}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("shared cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(SharedDistributedCacheRuntime::new()))
        .clone()
}

#[derive(Debug, Clone)]
struct LocalCacheBackendAdapter {
    state: CacheBackendState,
}

impl LocalCacheBackendAdapter {
    fn new() -> Self {
        Self {
            state: CacheBackendState::new(),
        }
    }

    fn insert(&mut self, entry: CacheEntry) {
        self.state.insert(entry);
    }

    fn lookup(&mut self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.state.lookup(key, now)
    }

    fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.state.invalidate(tags)
    }

    fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        self.state.begin_fill(key, mode, holder)
    }

    fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.state.complete_fill(lease)
    }

    fn metrics(&self) -> CacheMetrics {
        self.state.metrics
    }
}

#[derive(Debug, Clone)]
enum CacheBackendStorage {
    Local(LocalCacheBackendAdapter),
    Distributed(DistributedCacheClient),
}

#[derive(Debug, Clone)]
pub struct CacheBackendAdapter {
    kind: CacheBackendKind,
    topology: CacheTopology,
    storage: CacheBackendStorage,
}

impl CacheBackendAdapter {
    pub fn new(topology: CacheTopology) -> Self {
        Self::shared(topology)
    }

    pub fn in_memory(topology: CacheTopology) -> Self {
        let kind = match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
            Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
            None => CacheBackendKind::Local,
        };

        Self {
            kind,
            topology,
            storage: CacheBackendStorage::Local(LocalCacheBackendAdapter::new()),
        }
    }

    pub fn distributed(topology: CacheTopology, client: DistributedCacheClient) -> Self {
        Self {
            kind: client.kind(),
            topology,
            storage: CacheBackendStorage::Distributed(client),
        }
    }

    pub fn shared(topology: CacheTopology) -> Self {
        Self::scoped_shared(topology, "global")
    }

    pub fn scoped_shared(topology: CacheTopology, scope: impl Into<String>) -> Self {
        let kind = match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
            Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
            None => CacheBackendKind::Local,
        };
        let scope = scope.into();
        let storage = if topology.supports_shared_invalidation() {
            CacheBackendStorage::Distributed(DistributedCacheClient::scoped_shared(kind, scope))
        } else {
            CacheBackendStorage::Local(LocalCacheBackendAdapter::new())
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
        match &self.storage {
            CacheBackendStorage::Local(_) => false,
            CacheBackendStorage::Distributed(client) => client.is_shared(),
        }
    }

    pub fn insert(&mut self, entry: CacheEntry) {
        match &mut self.storage {
            CacheBackendStorage::Local(adapter) => adapter.insert(entry),
            CacheBackendStorage::Distributed(client) => client.insert(entry),
        }
    }

    pub fn lookup(&mut self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        match &mut self.storage {
            CacheBackendStorage::Local(adapter) => adapter.lookup(key, now),
            CacheBackendStorage::Distributed(client) => client.lookup(key, now),
        }
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        match &mut self.storage {
            CacheBackendStorage::Local(adapter) => adapter.invalidate(tags),
            CacheBackendStorage::Distributed(client) => client.invalidate(tags),
        }
    }

    pub fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        match &mut self.storage {
            CacheBackendStorage::Local(adapter) => adapter.begin_fill(key, mode, holder),
            CacheBackendStorage::Distributed(client) => client.begin_fill(key, mode, holder),
        }
    }

    pub fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        match &mut self.storage {
            CacheBackendStorage::Local(adapter) => adapter.complete_fill(lease),
            CacheBackendStorage::Distributed(client) => client.complete_fill(lease),
        }
    }

    pub fn metrics(&self) -> CacheMetrics {
        match &self.storage {
            CacheBackendStorage::Local(adapter) => adapter.metrics(),
            CacheBackendStorage::Distributed(client) => client.metrics(),
        }
    }
}

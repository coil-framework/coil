use std::fmt;
use std::sync::Arc;

use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, CacheTopology,
    FillDecision, FillLease, InvalidationSet, RequestCoalescingMode,
};

mod client;
mod live;
mod local;
mod state;
mod testing;

pub use client::{DistributedCacheClient, DistributedCacheRuntime};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheBackendKind {
    Local,
    Redis,
    Valkey,
}

#[derive(Debug, Clone)]
enum CacheBackendStorage {
    Local(local::LocalCacheBackendAdapter),
    Distributed(DistributedCacheClient),
}

#[derive(Clone)]
pub struct CacheBackendAdapter {
    kind: CacheBackendKind,
    topology: CacheTopology,
    shared: bool,
    storage: CacheBackendStorage,
}

impl CacheBackendAdapter {
    pub fn new(topology: CacheTopology) -> Self {
        match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => Self::distributed(
                topology,
                DistributedCacheClient::with_shared_runtime(
                    CacheBackendKind::Redis,
                    DistributedCacheClient::unavailable_shared_runtime(CacheBackendKind::Redis),
                ),
            ),
            Some(crate::DistributedCacheBackend::Valkey) => Self::distributed(
                topology,
                DistributedCacheClient::with_shared_runtime(
                    CacheBackendKind::Valkey,
                    DistributedCacheClient::unavailable_shared_runtime(CacheBackendKind::Valkey),
                ),
            ),
            None => Self::local_backend(topology),
        }
    }

    #[cfg(test)]
    pub fn local_for_testing(topology: CacheTopology) -> Self {
        Self::local_backend(topology)
    }

    fn local_backend(topology: CacheTopology) -> Self {
        let kind = match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
            Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
            None => CacheBackendKind::Local,
        };

        Self {
            kind,
            topology,
            shared: false,
            storage: CacheBackendStorage::Local(local::LocalCacheBackendAdapter::new()),
        }
    }

    pub fn distributed(topology: CacheTopology, client: DistributedCacheClient) -> Self {
        Self {
            kind: client.kind(),
            topology,
            shared: client.is_shared(),
            storage: CacheBackendStorage::Distributed(client),
        }
    }

    pub fn with_shared_runtime(
        topology: CacheTopology,
        runtime: Arc<dyn DistributedCacheRuntime>,
    ) -> Self {
        let client = DistributedCacheClient::with_shared_runtime(
            match topology.l2() {
                Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
                Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
                None => CacheBackendKind::Local,
            },
            runtime,
        );
        let kind = match topology.l2() {
            Some(crate::DistributedCacheBackend::Redis) => CacheBackendKind::Redis,
            Some(crate::DistributedCacheBackend::Valkey) => CacheBackendKind::Valkey,
            None => CacheBackendKind::Local,
        };
        Self {
            kind,
            topology,
            shared: client.is_shared(),
            storage: CacheBackendStorage::Distributed(client),
        }
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[cfg(test)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(topology). use with_shared_runtime(topology, runtime) or local_for_testing(topology)"
    )]
    pub fn shared(topology: CacheTopology) -> Self {
        Self::local_for_testing(topology)
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[cfg(test)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(topology). use with_shared_runtime(topology, runtime) or local_for_testing(topology)"
    )]
    pub fn scoped_shared(topology: CacheTopology, _scope: impl Into<String>) -> Self {
        Self::local_for_testing(topology)
    }

    pub fn kind(&self) -> CacheBackendKind {
        self.kind
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn is_shared(&self) -> bool {
        self.shared
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

impl fmt::Debug for CacheBackendAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("CacheBackendAdapter");
        debug.field("kind", &self.kind);
        debug.field("topology", &self.topology);
        debug.field("shared", &self.shared);
        debug.finish_non_exhaustive()
    }
}

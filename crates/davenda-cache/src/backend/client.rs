use std::fmt;
use std::sync::Arc;

use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision,
    FillLease, InvalidationSet, RequestCoalescingMode,
};

use super::{CacheBackendKind, shared};

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
    fn is_shared_backend(&self) -> bool {
        true
    }
    fn supports_live_shared_state(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct DistributedCacheClient {
    kind: CacheBackendKind,
    shared: bool,
    runtime: Arc<dyn DistributedCacheRuntime>,
}

impl DistributedCacheClient {
    pub fn new(kind: CacheBackendKind, runtime: Arc<dyn DistributedCacheRuntime>) -> Self {
        Self::with_runtime(kind, runtime)
    }

    pub fn with_runtime(kind: CacheBackendKind, runtime: Arc<dyn DistributedCacheRuntime>) -> Self {
        Self::with_shared_runtime(kind, runtime)
    }

    pub fn with_shared_runtime(
        kind: CacheBackendKind,
        runtime: Arc<dyn DistributedCacheRuntime>,
    ) -> Self {
        Self {
            kind,
            shared: runtime.is_shared_backend(),
            runtime,
        }
    }

    #[doc(hidden)]
    pub fn emulated_shared_runtime(kind: CacheBackendKind) -> Arc<dyn DistributedCacheRuntime> {
        let _ = kind;
        super::testing::shared_test_runtime(kind, super::testing::test_scope_namespace())
    }

    pub fn persistent_shared_runtime(
        kind: CacheBackendKind,
        namespace: impl Into<String>,
    ) -> Arc<dyn DistributedCacheRuntime> {
        shared::persistent_runtime(kind, namespace.into())
    }

    #[cfg(test)]
    #[doc(hidden)]
    pub fn local_for_testing(kind: CacheBackendKind) -> Self {
        Self::with_shared_runtime(kind, Self::emulated_shared_runtime(kind))
    }

    #[cfg(test)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(kind). use with_shared_runtime(kind, runtime) or local_for_testing(kind)"
    )]
    pub fn shared(kind: CacheBackendKind) -> Self {
        Self::local_for_testing(kind)
    }

    #[cfg(test)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(kind). use with_shared_runtime(kind, runtime) or local_for_testing(kind)"
    )]
    pub fn scoped_shared(kind: CacheBackendKind, _scope: impl Into<String>) -> Self {
        Self::local_for_testing(kind)
    }

    pub fn kind(&self) -> CacheBackendKind {
        self.kind
    }

    pub fn is_shared(&self) -> bool {
        self.shared
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

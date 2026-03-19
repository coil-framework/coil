use super::*;
use davenda_cache::CacheBackendKind;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeBackendMaterializer {
    scope: String,
    plans: SharedBackendClients,
    session_runtimes: Arc<SessionRuntimeRegistry>,
}

impl RuntimeBackendMaterializer {
    pub(crate) fn new(scope: String, plans: SharedBackendClients) -> Self {
        Self {
            scope,
            plans,
            session_runtimes: Arc::new(SessionRuntimeRegistry::default()),
        }
    }

    pub(crate) fn browser_host(
        &self,
        customer_app: String,
        services: BrowserSecurityServices,
    ) -> Result<BrowserHost, BrowserHostBuildError> {
        match self.plans.session_store.as_ref() {
            Some(target) => BrowserHost::with_session_store_client(
                customer_app.clone(),
                services.clone(),
                DistributedSessionStoreClient::new(
                    target.kind,
                    self.session_runtimes
                        .runtime_for(target.kind, format!("{}:{customer_app}", self.scope)),
                ),
            ),
            None => Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost),
        }
    }

    pub(crate) fn cache_runtime(&self, planner: CachePlanner) -> CacheRuntime {
        match self.plans.distributed_cache.as_ref() {
            Some(target) => CacheRuntime::with_shared_runtime(
                planner.topology(),
                crate::cache::shared_distributed_runtime(
                    planner.topology(),
                    cache_backend_kind(target.backend),
                    self.scope.clone(),
                ),
            ),
            None => planner.runtime(),
        }
    }

    pub(crate) fn jobs_coordinator(
        &self,
        _customer_app: &str,
        runtime: &JobsRuntimeServices,
    ) -> JobsCoordinator {
        runtime.coordinator_with_shared_runtime(crate::jobs::shared_coordinator_runtime(
            runtime,
            self.scope.clone(),
        ))
    }
}

fn cache_backend_kind(backend: davenda_cache::DistributedCacheBackend) -> CacheBackendKind {
    match backend {
        davenda_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        davenda_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    }
}

#[derive(Default)]
struct SessionRuntimeRegistry {
    runtimes: Mutex<BTreeMap<String, Arc<dyn DistributedSessionStoreRuntime>>>,
}

impl SessionRuntimeRegistry {
    fn runtime_for(
        &self,
        kind: SessionStoreBackendKind,
        scope: String,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        let key = format!("{kind:?}:{scope}");
        let mut guard = self
            .runtimes
            .lock()
            .expect("session runtime registry mutex poisoned");
        guard
            .entry(key)
            .or_insert_with(|| DistributedSessionStoreClient::shared_runtime(kind))
            .clone()
    }
}

impl std::fmt::Debug for SessionRuntimeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRuntimeRegistry")
            .finish_non_exhaustive()
    }
}

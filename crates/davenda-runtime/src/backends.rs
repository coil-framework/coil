use super::*;
use davenda_cache::CacheBackendKind;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct RuntimeBackendMaterializer {
    scope: String,
    plans: SharedBackendClients,
    session_runtimes: Arc<SessionRuntimeRegistry>,
    cache_runtime: Option<Arc<dyn davenda_cache::DistributedCacheRuntime>>,
    jobs_runtime: Arc<Mutex<Option<Arc<dyn davenda_jobs::JobsCoordinationRuntime>>>>,
}

impl RuntimeBackendMaterializer {
    pub(crate) fn new(scope: String, plans: SharedBackendClients) -> Self {
        let cache_runtime = plans.distributed_cache.as_ref().map(|target| {
            davenda_cache::DistributedCacheClient::emulated_shared_runtime(cache_backend_kind(
                target.backend,
            ))
        });

        Self {
            scope,
            plans,
            session_runtimes: Arc::new(SessionRuntimeRegistry::default()),
            cache_runtime,
            jobs_runtime: Arc::new(Mutex::new(None)),
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
        if planner.topology().supports_shared_invalidation() {
            self.cache_runtime
                .as_ref()
                .map(|runtime| {
                    CacheRuntime::with_shared_runtime(planner.topology(), runtime.clone())
                })
                .unwrap_or_else(|| planner.runtime())
        } else {
            planner.runtime()
        }
    }

    pub(crate) fn jobs_coordinator(
        &self,
        _customer_app: &str,
        runtime: &JobsRuntimeServices,
    ) -> JobsCoordinator {
        let shared_runtime = {
            let mut guard = self
                .jobs_runtime
                .lock()
                .expect("shared jobs runtime mutex poisoned");
            guard
                .get_or_insert_with(|| {
                    davenda_jobs::JobsBackendAdapter::emulated_shared_runtime(runtime)
                })
                .clone()
        };

        runtime.coordinator_with_shared_runtime(shared_runtime)
    }
}

impl fmt::Debug for RuntimeBackendMaterializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeBackendMaterializer")
            .field("scope", &self.scope)
            .field("plans", &self.plans)
            .field("session_runtimes", &self.session_runtimes)
            .field(
                "cache_runtime",
                &self.cache_runtime.as_ref().map(|_| "shared"),
            )
            .field(
                "jobs_runtime",
                &self.jobs_runtime.lock().ok().map(|guard| guard.is_some()),
            )
            .finish_non_exhaustive()
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

impl fmt::Debug for SessionRuntimeRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionRuntimeRegistry")
            .finish_non_exhaustive()
    }
}

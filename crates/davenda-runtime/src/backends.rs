use super::*;
use davenda_cache::CacheBackendKind;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct RuntimeBackendMaterializer {
    namespace: String,
    plans: SharedBackendClients,
    #[cfg(test)]
    cache_runtime: Option<Arc<dyn davenda_cache::DistributedCacheRuntime>>,
    jobs_runtime: Arc<Mutex<Option<Arc<dyn davenda_jobs::JobsCoordinationRuntime>>>>,
}

impl RuntimeBackendMaterializer {
    pub(crate) fn new(namespace: String, plans: SharedBackendClients) -> Self {
        #[cfg(test)]
        let cache_runtime = plans.distributed_cache.as_ref().map(|target| {
            crate::plan::shared_cache_runtime_for_test(
                cache_backend_kind(target.backend),
                namespace.clone(),
            )
        });

        Self {
            namespace,
            plans,
            #[cfg(test)]
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
                    DistributedSessionStoreClient::shared_runtime(
                        target.kind,
                        format!("{}:{customer_app}", self.namespace),
                    ),
                ),
            ),
            None => Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
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

    #[cfg(test)]
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
                    crate::plan::shared_jobs_runtime_for_test(runtime, self.namespace.clone())
                })
                .clone()
        };

        runtime.coordinator_with_shared_runtime(shared_runtime)
    }
}

impl fmt::Debug for RuntimeBackendMaterializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("RuntimeBackendMaterializer");
        debug.field("namespace", &self.namespace);
        debug.field("plans", &self.plans);
        #[cfg(test)]
        debug.field(
            "cache_runtime",
            &self.cache_runtime.as_ref().map(|_| "shared"),
        );
        debug.field(
            "jobs_runtime",
            &self.jobs_runtime.lock().ok().map(|guard| guard.is_some()),
        );
        debug.finish_non_exhaustive()
    }
}

#[cfg(test)]
fn cache_backend_kind(backend: davenda_cache::DistributedCacheBackend) -> CacheBackendKind {
    match backend {
        davenda_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        davenda_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    }
}

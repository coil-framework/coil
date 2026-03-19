use super::*;
use davenda_cache::{CacheBackendAdapter, CacheBackendKind, DistributedCacheClient};
use davenda_jobs::JobsBackendAdapter;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeBackendMaterializer {
    scope: String,
    plans: SharedBackendClients,
}

impl RuntimeBackendMaterializer {
    pub(crate) fn new(scope: String, plans: SharedBackendClients) -> Self {
        Self { scope, plans }
    }

    pub(crate) fn browser_host(
        &self,
        customer_app: String,
        services: BrowserSecurityServices,
    ) -> BrowserHost {
        match self.plans.session_store.as_ref() {
            Some(target) => BrowserHost::with_session_store_client(
                customer_app.clone(),
                services.clone(),
                DistributedSessionStoreClient::shared(target.kind, self.scope.clone()),
            )
            .expect("materialized session store target must match browser services"),
            None => BrowserHost::new_with_scope(customer_app, services, self.scope.clone()),
        }
    }

    pub(crate) fn cache_runtime(&self, planner: CachePlanner) -> CacheRuntime {
        match self.plans.distributed_cache.as_ref() {
            Some(target) => CacheRuntime::with_backend(
                planner.topology(),
                CacheBackendAdapter::distributed(
                    planner.topology(),
                    DistributedCacheClient::scoped_shared(
                        cache_backend_kind(target.backend),
                        self.scope.clone(),
                    ),
                ),
            ),
            None => planner.runtime(),
        }
    }

    pub(crate) fn jobs_coordinator(
        &self,
        customer_app: &str,
        runtime: &JobsRuntimeServices,
    ) -> JobsCoordinator {
        let backend = if self.plans.jobs.shared {
            JobsBackendAdapter::shared_scoped(runtime, format!("{}:{customer_app}", self.scope))
        } else {
            JobsBackendAdapter::in_memory(runtime)
        };
        runtime.coordinator_with_backend(backend)
    }
}

fn cache_backend_kind(backend: davenda_cache::DistributedCacheBackend) -> CacheBackendKind {
    match backend {
        davenda_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        davenda_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    }
}

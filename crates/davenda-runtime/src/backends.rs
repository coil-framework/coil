use super::*;
use davenda_cache::{CacheBackendAdapter, CacheBackendKind, DistributedCacheClient};
use davenda_jobs::JobsBackendAdapter;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

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
                shared_cache_backend(
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
        customer_app: &str,
        runtime: &JobsRuntimeServices,
    ) -> JobsCoordinator {
        let backend = shared_jobs_backend(runtime, format!("{}:{customer_app}", self.scope));
        runtime.coordinator_with_backend(backend)
    }
}

pub(crate) fn shared_cache_backend(
    topology: CacheTopology,
    backend: CacheBackendKind,
    scope: String,
) -> CacheBackendAdapter {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, CacheBackendAdapter>>> = OnceLock::new();

    let key = format!("{topology:?}:{backend:?}:{scope}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("shared cache backend registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            CacheBackendAdapter::distributed(
                topology,
                DistributedCacheClient::scoped_shared(backend, scope.clone()),
            )
        })
        .clone()
}

pub(crate) fn shared_jobs_backend(
    runtime: &JobsRuntimeServices,
    scope: String,
) -> JobsBackendAdapter {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, JobsBackendAdapter>>> = OnceLock::new();

    let key = format!("{runtime:?}:{scope}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("shared jobs backend registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| JobsBackendAdapter::shared_scoped(runtime, scope.clone()))
        .clone()
}

fn cache_backend_kind(backend: davenda_cache::DistributedCacheBackend) -> CacheBackendKind {
    match backend {
        davenda_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        davenda_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    }
}

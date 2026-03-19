use super::*;
use davenda_cache::{CacheBackendKind, DistributedCacheClient};
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
    ) -> Result<BrowserHost, BrowserHostBuildError> {
        match self.plans.session_store.as_ref() {
            Some(target) => BrowserHost::with_session_store_client(
                customer_app.clone(),
                services.clone(),
                DistributedSessionStoreClient::shared(target.kind, self.scope.clone()),
            ),
            None => BrowserHost::new_with_scope(customer_app, services, self.scope.clone()),
        }
    }

    pub(crate) fn cache_runtime(&self, planner: CachePlanner) -> CacheRuntime {
        match self.plans.distributed_cache.as_ref() {
            Some(target) => CacheRuntime::with_shared_runtime(
                planner.topology(),
                shared_cache_runtime(
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
        runtime.coordinator_with_shared_runtime(shared_jobs_runtime(
            runtime,
            format!("{}:{customer_app}", self.scope),
        ))
    }
}

pub(crate) fn shared_cache_runtime(
    topology: CacheTopology,
    backend: CacheBackendKind,
    scope: String,
) -> std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime> {
    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime>>>,
    > = OnceLock::new();

    let key = format!("{topology:?}:{backend:?}:{scope}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("shared cache runtime registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| DistributedCacheClient::emulated_shared_runtime(backend))
        .clone()
}

pub(crate) fn shared_jobs_runtime(
    runtime: &JobsRuntimeServices,
    scope: String,
) -> std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime> {
    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime>>>,
    > = OnceLock::new();

    let key = format!("{runtime:?}:{scope}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("shared jobs runtime registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| davenda_jobs::JobsBackendAdapter::emulated_shared_runtime(runtime))
        .clone()
}

fn cache_backend_kind(backend: davenda_cache::DistributedCacheBackend) -> CacheBackendKind {
    match backend {
        davenda_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        davenda_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    }
}

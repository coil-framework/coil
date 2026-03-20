#![cfg_attr(test, allow(dead_code))]

#[cfg(test)]
use super::EmulatedJobsCoordinationRuntime;
#[cfg(test)]
use super::JobsCoordinationRuntime;
use super::JobsRuntime;
use crate::JobsModelError;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
mod harness;

#[cfg(test)]
use harness::SharedJobsRuntimeHarness;

#[cfg(test)]
pub(crate) fn test_only_sqlite_shared_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    test_only_sqlite_shared_runtime_impl(runtime, namespace.into())
}

#[cfg(test)]
fn test_only_sqlite_shared_runtime_impl(
    runtime: &JobsRuntime,
    namespace: String,
) -> Arc<dyn JobsCoordinationRuntime> {
    static REGISTRY: OnceLock<
        Mutex<std::collections::BTreeMap<String, Arc<dyn JobsCoordinationRuntime>>>,
    > = OnceLock::new();

    let key = format!(
        "{}:{:?}:{}:{}:{}:{}:{}",
        test_scope(),
        runtime.backend,
        runtime.topology.work_queue.as_str(),
        runtime.topology.scheduled_queue.as_str(),
        runtime.topology.domain_events_queue.as_str(),
        runtime.topology.dead_letter_queue.as_str(),
        namespace
    );
    let registry = REGISTRY.get_or_init(|| Mutex::new(std::collections::BTreeMap::new()));
    let mut guard = registry.lock().expect("test jobs registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedJobsRuntimeHarness::new(Arc::new(
                EmulatedJobsCoordinationRuntime::new(runtime.clone()),
            )))
        })
        .clone()
}

#[cfg(test)]
fn test_scope() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test")
        .to_string()
}

pub(crate) fn explicit_distributed_backend_error(runtime: &JobsRuntime) -> JobsModelError {
    let namespace = std::env::var("DAVENDA_SHARED_BACKEND_NAMESPACE").unwrap_or_else(|_| {
        format!(
            "jobs:{:?}:{}:{}:{}:{}:{}",
            runtime.backend,
            runtime.topology.work_queue.as_str(),
            runtime.topology.scheduled_queue.as_str(),
            runtime.topology.domain_events_queue.as_str(),
            runtime.topology.dead_letter_queue.as_str(),
            std::process::id()
        )
    });

    JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
        backend: runtime.backend,
        namespace,
    }
}

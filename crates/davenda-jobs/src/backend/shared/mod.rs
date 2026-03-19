#![cfg_attr(test, allow(dead_code))]

#[cfg(test)]
use super::EmulatedJobsCoordinationRuntime;
use super::{JobsCoordinationRuntime, JobsRuntime};
use std::sync::Arc;
#[cfg(not(test))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

mod runtime;
mod store;

#[cfg(test)]
mod harness;

#[cfg(test)]
use harness::SharedJobsRuntimeHarness;
#[cfg(not(test))]
use runtime::PersistentJobsCoordinationRuntime;

#[cfg(not(test))]
static LOCAL_NAMESPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn persistent_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    shared_test_runtime(runtime, namespace.into())
}

#[cfg(not(test))]
pub(crate) fn local_runtime(runtime: &JobsRuntime) -> Arc<dyn JobsCoordinationRuntime> {
    persistent_runtime(runtime, default_namespace(runtime))
}

#[cfg(test)]
fn shared_test_runtime(
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

#[cfg(not(test))]
pub(crate) fn persistent_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(PersistentJobsCoordinationRuntime::new(
        runtime.clone(),
        namespace.into(),
    ))
}

#[cfg(not(test))]
pub(crate) fn default_namespace(runtime: &JobsRuntime) -> String {
    if let Ok(namespace) = std::env::var("DAVENDA_SHARED_BACKEND_NAMESPACE") {
        return namespace;
    }

    format!(
        "jobs:{:?}:{}:{}:{}:{}:{}:{}",
        runtime.backend,
        runtime.topology.work_queue.as_str(),
        runtime.topology.scheduled_queue.as_str(),
        runtime.topology.domain_events_queue.as_str(),
        runtime.topology.dead_letter_queue.as_str(),
        std::process::id(),
        LOCAL_NAMESPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
fn test_scope() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test")
        .to_string()
}

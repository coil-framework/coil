#![cfg_attr(test, allow(dead_code))]

#[cfg(test)]
use super::EmulatedJobsCoordinationRuntime;
use super::{JobsCoordinationRuntime, JobsRuntime};
#[cfg(not(test))]
use crate::{
    DeadLetterReason, JobFailureDisposition, JobId, JobInstant, JobLease, JobQueueName, JobSpec,
    JobsCoordinatorSnapshot, JobsModelError, SchedulerLeadership,
};
#[cfg(not(test))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
#[cfg(not(test))]
use std::time::Duration;

#[cfg(test)]
mod runtime;
#[cfg(test)]
mod store;

#[cfg(test)]
mod harness;

#[cfg(test)]
use harness::SharedJobsRuntimeHarness;

#[cfg(not(test))]
static LOCAL_NAMESPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn test_only_persistent_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    shared_test_runtime(runtime, namespace.into())
}

#[cfg(not(test))]
pub(crate) fn unconfigured_live_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(UnconfiguredJobsCoordinationRuntime::new(
        runtime.clone(),
        namespace.into(),
    ))
}

#[cfg(not(test))]
pub(crate) fn local_runtime(runtime: &JobsRuntime) -> Arc<dyn JobsCoordinationRuntime> {
    // Live jobs coordination must be configured explicitly; this path only
    // constructs the rejection backend for non-test builds.
    unconfigured_live_runtime(runtime, default_namespace(runtime))
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

#[cfg(not(test))]
#[derive(Debug)]
struct UnconfiguredJobsCoordinationRuntime {
    runtime: JobsRuntime,
    namespace: String,
}

#[cfg(not(test))]
impl UnconfiguredJobsCoordinationRuntime {
    fn new(runtime: JobsRuntime, namespace: String) -> Self {
        Self { runtime, namespace }
    }

    fn unsupported_message(&self) -> String {
        format!(
            "live shared jobs backend `{backend:?}` for `{namespace}` requires an explicit distributed runtime; file-backed shared state is test-only",
            backend = self.runtime.backend,
            namespace = self.namespace
        )
    }
}

#[cfg(not(test))]
impl JobsCoordinationRuntime for UnconfiguredJobsCoordinationRuntime {
    fn snapshot(&self) -> JobsCoordinatorSnapshot {
        panic!("{}", self.unsupported_message());
    }

    fn enqueue(&self, _spec: JobSpec, _now: JobInstant) -> Result<(), JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn acquire_scheduler_leadership(
        &self,
        _node_id: String,
        _now: JobInstant,
        _lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn promote_due_jobs(
        &self,
        _node_id: &str,
        _now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn lease_ready_jobs(
        &self,
        _queue: &JobQueueName,
        _worker_id: String,
        _now: JobInstant,
        _lease_ttl: Duration,
        _max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn acknowledge_completed(
        &self,
        _lease: &JobLease,
        _now: JobInstant,
    ) -> Result<(), JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn acknowledge_failed(
        &self,
        _lease: &JobLease,
        _now: JobInstant,
        _reason: DeadLetterReason,
        _error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn is_shared_backend(&self) -> bool {
        false
    }

    fn supports_live_shared_state(&self) -> bool {
        false
    }
}

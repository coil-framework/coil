use std::sync::Arc;
use std::time::Duration;

use crate::JobsRuntimeServices;
use davenda_cache::{
    CacheBackendKind, CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics,
    CacheModelError, DistributedCacheRuntime, FillDecision, FillLease, InvalidationSet,
    RequestCoalescingMode,
};
use davenda_jobs::{
    DeadLetterReason, JobFailureDisposition, JobId, JobInstant, JobLease, JobQueueName, JobSpec,
    JobsCoordinationRuntime, JobsCoordinatorSnapshot, JobsModelError, SchedulerLeadership,
};

pub(crate) fn live_rejection_jobs_runtime(
    runtime: &JobsRuntimeServices,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(LiveRejectionJobsRuntime::new(
        runtime.backend,
        namespace.into(),
    ))
}

pub(crate) fn live_rejection_cache_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
) -> Arc<dyn DistributedCacheRuntime> {
    Arc::new(LiveRejectionCacheRuntime::new(kind, namespace.into()))
}

#[derive(Debug)]
struct LiveRejectionJobsRuntime {
    backend: davenda_config::JobBackend,
    namespace: String,
}

impl LiveRejectionJobsRuntime {
    fn new(backend: davenda_config::JobBackend, namespace: String) -> Self {
        Self { backend, namespace }
    }

    fn unsupported_message(&self) -> String {
        format!(
            "live shared jobs backend `{backend:?}` for `{namespace}` requires an explicit distributed runtime; file-backed shared state is test-only",
            backend = self.backend,
            namespace = self.namespace
        )
    }
}

impl JobsCoordinationRuntime for LiveRejectionJobsRuntime {
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
}

#[derive(Debug)]
struct LiveRejectionCacheRuntime {
    kind: CacheBackendKind,
    namespace: String,
}

impl LiveRejectionCacheRuntime {
    fn new(kind: CacheBackendKind, namespace: String) -> Self {
        Self { kind, namespace }
    }

    fn unsupported_message(&self) -> String {
        format!(
            "live shared cache backend `{kind:?}` for `{namespace}` requires an explicit distributed runtime; file-backed shared state is test-only",
            kind = self.kind,
            namespace = self.namespace
        )
    }
}

impl DistributedCacheRuntime for LiveRejectionCacheRuntime {
    fn insert(&self, _entry: CacheEntry) {
        panic!("{}", self.unsupported_message());
    }

    fn lookup(&self, _key: &CacheKey, _now: CacheInstant) -> CacheLookup {
        panic!("{}", self.unsupported_message());
    }

    fn invalidate(&self, _tags: &InvalidationSet) -> Vec<CacheKey> {
        panic!("{}", self.unsupported_message());
    }

    fn begin_fill(
        &self,
        _key: &CacheKey,
        _mode: RequestCoalescingMode,
        _holder: String,
    ) -> FillDecision {
        panic!("{}", self.unsupported_message());
    }

    fn complete_fill(&self, _lease: &FillLease) -> Result<(), CacheModelError> {
        panic!("{}", self.unsupported_message());
    }

    fn metrics(&self) -> CacheMetrics {
        panic!("{}", self.unsupported_message());
    }

    fn is_shared_backend(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_config::{JobBackend, JobsConfig};
    use davenda_jobs::JobsRuntime;

    #[test]
    fn live_rejection_jobs_runtime_reports_non_shared_state() {
        let runtime = JobsRuntime::from_config(&JobsConfig {
            backend: JobBackend::Redis,
            retry_limit: 5,
        })
        .expect("test jobs runtime");

        let shared_runtime = live_rejection_jobs_runtime(&runtime, "test-ns");
        assert!(!shared_runtime.is_shared_backend());
    }

    #[test]
    fn live_rejection_cache_runtime_reports_non_shared_state() {
        let shared_runtime = live_rejection_cache_runtime(CacheBackendKind::Redis, "test-ns");
        assert!(!shared_runtime.is_shared_backend());
    }
}

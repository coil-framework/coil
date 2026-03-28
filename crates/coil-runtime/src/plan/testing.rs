use super::*;

#[cfg(test)]
pub(crate) fn shared_cache_runtime_for_test(
    backend: coil_cache::CacheBackendKind,
    namespace: String,
) -> std::sync::Arc<dyn coil_cache::DistributedCacheRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn coil_cache::DistributedCacheRuntime>>>,
    > = OnceLock::new();

    let key = format!("{backend:?}:{namespace}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedCacheRuntimeHarness::new(
                coil_cache::DistributedCacheClient::emulated_shared_runtime(backend),
            ))
        })
        .clone()
}

#[cfg(test)]
pub(crate) fn shared_jobs_runtime_for_test(
    runtime: &JobsRuntimeServices,
    namespace: String,
) -> std::sync::Arc<dyn coil_jobs::JobsCoordinationRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn coil_jobs::JobsCoordinationRuntime>>>,
    > = OnceLock::new();

    let key = format!(
        "{:?}:{}:{}:{}:{}:{}",
        runtime.backend,
        runtime.topology.work_queue.as_str(),
        runtime.topology.scheduled_queue.as_str(),
        runtime.topology.domain_events_queue.as_str(),
        runtime.topology.dead_letter_queue.as_str(),
        namespace
    );
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test jobs registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedJobsRuntimeHarness::new(
                coil_jobs::JobsBackendAdapter::emulated_shared_runtime(runtime),
            ))
        })
        .clone()
}

#[cfg(test)]
#[derive(Clone)]
struct SharedCacheRuntimeHarness {
    runtime: std::sync::Arc<dyn coil_cache::DistributedCacheRuntime>,
}

#[cfg(test)]
impl SharedCacheRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn coil_cache::DistributedCacheRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl coil_cache::DistributedCacheRuntime for SharedCacheRuntimeHarness {
    fn insert(&self, entry: coil_cache::CacheEntry) {
        self.runtime.insert(entry);
    }

    fn lookup(
        &self,
        key: &coil_cache::CacheKey,
        now: coil_cache::CacheInstant,
    ) -> coil_cache::CacheLookup {
        self.runtime.lookup(key, now)
    }

    fn invalidate(&self, tags: &coil_cache::InvalidationSet) -> Vec<coil_cache::CacheKey> {
        self.runtime.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &coil_cache::CacheKey,
        mode: coil_cache::RequestCoalescingMode,
        holder: String,
    ) -> coil_cache::FillDecision {
        self.runtime.begin_fill(key, mode, holder)
    }

    fn complete_fill(
        &self,
        lease: &coil_cache::FillLease,
    ) -> Result<(), coil_cache::CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    fn metrics(&self) -> coil_cache::CacheMetrics {
        self.runtime.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[derive(Clone)]
struct SharedJobsRuntimeHarness {
    runtime: std::sync::Arc<dyn coil_jobs::JobsCoordinationRuntime>,
}

#[cfg(test)]
impl SharedJobsRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn coil_jobs::JobsCoordinationRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl coil_jobs::JobsCoordinationRuntime for SharedJobsRuntimeHarness {
    fn snapshot(&self) -> coil_jobs::JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    fn enqueue(
        &self,
        spec: coil_jobs::JobSpec,
        now: coil_jobs::JobInstant,
    ) -> Result<(), coil_jobs::JobsModelError> {
        self.runtime.enqueue(spec, now)
    }

    fn retry_dead_letter(
        &self,
        dead_letter_id: &coil_jobs::DeadLetterId,
        now: coil_jobs::JobInstant,
    ) -> Result<coil_jobs::QueuedJobRecord, coil_jobs::JobsModelError> {
        self.runtime.retry_dead_letter(dead_letter_id, now)
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: coil_jobs::JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<coil_jobs::SchedulerLeadership, coil_jobs::JobsModelError> {
        self.runtime
            .acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: coil_jobs::JobInstant,
    ) -> Result<Vec<coil_jobs::JobId>, coil_jobs::JobsModelError> {
        self.runtime.promote_due_jobs(node_id, now)
    }

    fn lease_ready_jobs(
        &self,
        queue: &coil_jobs::JobQueueName,
        worker_id: String,
        now: coil_jobs::JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<coil_jobs::JobLease>, coil_jobs::JobsModelError> {
        self.runtime
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
    }

    fn acknowledge_completed(
        &self,
        lease: &coil_jobs::JobLease,
        now: coil_jobs::JobInstant,
    ) -> Result<(), coil_jobs::JobsModelError> {
        self.runtime.acknowledge_completed(lease, now)
    }

    fn acknowledge_failed(
        &self,
        lease: &coil_jobs::JobLease,
        now: coil_jobs::JobInstant,
        reason: coil_jobs::DeadLetterReason,
        error_message: String,
    ) -> Result<coil_jobs::JobFailureDisposition, coil_jobs::JobsModelError> {
        self.runtime
            .acknowledge_failed(lease, now, reason, error_message)
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

use super::*;

#[cfg(test)]
pub(crate) fn shared_cache_runtime_for_test(
    backend: davenda_cache::CacheBackendKind,
    namespace: String,
) -> std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn davenda_cache::DistributedCacheRuntime>>>,
    > = OnceLock::new();

    let key = format!("{backend:?}:{namespace}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedCacheRuntimeHarness::new(
                davenda_cache::DistributedCacheClient::emulated_shared_runtime(backend),
            ))
        })
        .clone()
}

#[cfg(test)]
pub(crate) fn shared_jobs_runtime_for_test(
    runtime: &JobsRuntimeServices,
    namespace: String,
) -> std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn davenda_jobs::JobsCoordinationRuntime>>>,
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
                davenda_jobs::JobsBackendAdapter::emulated_shared_runtime(runtime),
            ))
        })
        .clone()
}

#[cfg(test)]
#[derive(Clone)]
struct SharedCacheRuntimeHarness {
    runtime: std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime>,
}

#[cfg(test)]
impl SharedCacheRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl davenda_cache::DistributedCacheRuntime for SharedCacheRuntimeHarness {
    fn insert(&self, entry: davenda_cache::CacheEntry) {
        self.runtime.insert(entry);
    }

    fn lookup(
        &self,
        key: &davenda_cache::CacheKey,
        now: davenda_cache::CacheInstant,
    ) -> davenda_cache::CacheLookup {
        self.runtime.lookup(key, now)
    }

    fn invalidate(&self, tags: &davenda_cache::InvalidationSet) -> Vec<davenda_cache::CacheKey> {
        self.runtime.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &davenda_cache::CacheKey,
        mode: davenda_cache::RequestCoalescingMode,
        holder: String,
    ) -> davenda_cache::FillDecision {
        self.runtime.begin_fill(key, mode, holder)
    }

    fn complete_fill(
        &self,
        lease: &davenda_cache::FillLease,
    ) -> Result<(), davenda_cache::CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    fn metrics(&self) -> davenda_cache::CacheMetrics {
        self.runtime.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[derive(Clone)]
struct SharedJobsRuntimeHarness {
    runtime: std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime>,
}

#[cfg(test)]
impl SharedJobsRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl davenda_jobs::JobsCoordinationRuntime for SharedJobsRuntimeHarness {
    fn snapshot(&self) -> davenda_jobs::JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    fn enqueue(
        &self,
        spec: davenda_jobs::JobSpec,
        now: davenda_jobs::JobInstant,
    ) -> Result<(), davenda_jobs::JobsModelError> {
        self.runtime.enqueue(spec, now)
    }

    fn retry_dead_letter(
        &self,
        dead_letter_id: &davenda_jobs::DeadLetterId,
        now: davenda_jobs::JobInstant,
    ) -> Result<davenda_jobs::QueuedJobRecord, davenda_jobs::JobsModelError> {
        self.runtime.retry_dead_letter(dead_letter_id, now)
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: davenda_jobs::JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<davenda_jobs::SchedulerLeadership, davenda_jobs::JobsModelError> {
        self.runtime
            .acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: davenda_jobs::JobInstant,
    ) -> Result<Vec<davenda_jobs::JobId>, davenda_jobs::JobsModelError> {
        self.runtime.promote_due_jobs(node_id, now)
    }

    fn lease_ready_jobs(
        &self,
        queue: &davenda_jobs::JobQueueName,
        worker_id: String,
        now: davenda_jobs::JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<davenda_jobs::JobLease>, davenda_jobs::JobsModelError> {
        self.runtime
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
    }

    fn acknowledge_completed(
        &self,
        lease: &davenda_jobs::JobLease,
        now: davenda_jobs::JobInstant,
    ) -> Result<(), davenda_jobs::JobsModelError> {
        self.runtime.acknowledge_completed(lease, now)
    }

    fn acknowledge_failed(
        &self,
        lease: &davenda_jobs::JobLease,
        now: davenda_jobs::JobInstant,
        reason: davenda_jobs::DeadLetterReason,
        error_message: String,
    ) -> Result<davenda_jobs::JobFailureDisposition, davenda_jobs::JobsModelError> {
        self.runtime
            .acknowledge_failed(lease, now, reason, error_message)
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

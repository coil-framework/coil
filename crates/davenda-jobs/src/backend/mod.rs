use crate::error::JobsModelError;
use crate::identifiers::{DeadLetterId, IdempotencyKey, JobId, JobQueueName};
use crate::model::{DeadLetterOutcome, DeadLetterReason, JobInstant, QueueTopology};
use crate::runtime::{JobSpec, JobsRuntime};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

mod shared;
mod live;
mod state;
mod testing;

use state::JobsBackendState;
use testing::EmulatedJobsCoordinationRuntime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobExecutionContext {
    pub job_id: JobId,
    pub queue: JobQueueName,
    pub backend: davenda_config::JobBackend,
    pub idempotency_key: Option<IdempotencyKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedJobRecord {
    pub spec: JobSpec,
    pub attempts: u32,
    pub enqueued_at: JobInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobLease {
    pub record: QueuedJobRecord,
    pub worker_id: String,
    pub leased_at: JobInstant,
    pub lease_until: JobInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerLeadership {
    pub node_id: String,
    pub acquired_at: JobInstant,
    pub lease_until: JobInstant,
}

impl SchedulerLeadership {
    pub fn is_active(&self, now: JobInstant) -> bool {
        self.lease_until > now
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobFailureDisposition {
    Retried {
        job_id: JobId,
        next_attempt_at: JobInstant,
        queue: JobQueueName,
    },
    DeadLettered(DeadLetterOutcome),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct JobsCoordinatorSnapshot {
    pub ready: Vec<QueuedJobRecord>,
    pub scheduled: Vec<QueuedJobRecord>,
    pub in_flight: Vec<JobLease>,
    pub dead_letters: Vec<DeadLetterOutcome>,
    pub leadership: Option<SchedulerLeadership>,
}

pub trait JobsCoordinationRuntime: Send + Sync + 'static {
    fn snapshot(&self) -> JobsCoordinatorSnapshot;
    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError>;
    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError>;
    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError>;
    fn lease_ready_jobs(
        &self,
        queue: &JobQueueName,
        worker_id: String,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError>;
    fn acknowledge_completed(
        &self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError>;
    fn acknowledge_failed(
        &self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError>;
    fn is_shared_backend(&self) -> bool {
        true
    }
    fn supports_live_shared_state(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct JobsBackendAdapter {
    backend: davenda_config::JobBackend,
    queue_topology: QueueTopology,
    shared: bool,
    runtime: Arc<dyn JobsCoordinationRuntime>,
}

impl JobsBackendAdapter {
    pub fn new(
        backend: davenda_config::JobBackend,
        queue_topology: QueueTopology,
        runtime: Arc<dyn JobsCoordinationRuntime>,
    ) -> Self {
        Self::with_runtime(backend, queue_topology, runtime)
    }

    pub fn with_runtime(
        backend: davenda_config::JobBackend,
        queue_topology: QueueTopology,
        runtime: Arc<dyn JobsCoordinationRuntime>,
    ) -> Self {
        Self {
            backend,
            queue_topology,
            shared: runtime.is_shared_backend(),
            runtime,
        }
    }

    pub fn with_shared_runtime(
        backend: davenda_config::JobBackend,
        queue_topology: QueueTopology,
        runtime: Arc<dyn JobsCoordinationRuntime>,
    ) -> Self {
        Self::new(backend, queue_topology, runtime)
    }

    pub fn live_shared_runtime(
        runtime: &JobsRuntime,
        namespace: impl Into<String>,
        root: impl Into<std::path::PathBuf>,
    ) -> Arc<dyn JobsCoordinationRuntime> {
        live::live_shared_runtime(runtime, namespace, root)
    }

    pub fn emulated_shared_runtime(runtime: &JobsRuntime) -> Arc<dyn JobsCoordinationRuntime> {
        Arc::new(EmulatedJobsCoordinationRuntime::new(runtime.clone()))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    #[doc(hidden)]
    pub fn test_only_sqlite_shared_runtime(
        runtime: &JobsRuntime,
        namespace: impl Into<String>,
    ) -> Arc<dyn JobsCoordinationRuntime> {
        shared::test_only_sqlite_shared_runtime(runtime, namespace.into())
    }

    #[doc(hidden)]
    pub fn in_memory(runtime: &JobsRuntime) -> Result<Self, JobsModelError> {
        Self::local_for_testing(runtime)
    }

    #[doc(hidden)]
    pub fn local_for_testing(runtime: &JobsRuntime) -> Result<Self, JobsModelError> {
        #[cfg(test)]
        {
            let runtime_backend = Self::emulated_shared_runtime(runtime);
            return Ok(Self {
                backend: runtime.backend,
                queue_topology: runtime.topology.clone(),
                shared: false,
                runtime: runtime_backend,
            });
        }

        #[cfg(not(test))]
        {
            Err(explicit_distributed_backend_error(runtime))
        }
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(runtime). use with_shared_runtime(backend, topology, runtime) or local_for_testing(runtime)"
    )]
    pub fn shared(runtime: &JobsRuntime) -> Result<Self, JobsModelError> {
        Self::local_for_testing(runtime)
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(runtime). use with_shared_runtime(backend, topology, runtime) or local_for_testing(runtime)"
    )]
    pub fn shared_scoped(runtime: &JobsRuntime, _scope: impl Into<String>) -> Result<Self, JobsModelError> {
        Self::local_for_testing(runtime)
    }

    pub fn is_shared(&self) -> bool {
        self.shared
    }

    pub(crate) fn snapshot(&self) -> JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    pub(crate) fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.runtime.enqueue(spec, now)
    }

    pub(crate) fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        self.runtime
            .acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    pub(crate) fn promote_due_jobs(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        self.runtime.promote_due_jobs(node_id, now)
    }

    pub(crate) fn lease_ready_jobs(
        &self,
        queue: &JobQueueName,
        worker_id: String,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        self.runtime
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
    }

    pub(crate) fn acknowledge_completed(
        &self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.runtime.acknowledge_completed(lease, now)
    }

    pub(crate) fn acknowledge_failed(
        &self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        self.runtime
            .acknowledge_failed(lease, now, reason, error_message)
    }
}

pub(crate) fn explicit_distributed_backend_error(runtime: &JobsRuntime) -> JobsModelError {
    shared::explicit_distributed_backend_error(runtime)
}

impl fmt::Debug for JobsBackendAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobsBackendAdapter")
            .field("backend", &self.backend)
            .field("queue_topology", &self.queue_topology)
            .finish()
    }
}

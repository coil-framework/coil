use super::*;
use crate::backend::{JobFailureDisposition, JobLease, QueuedJobRecord, SchedulerLeadership};
use crate::error::JobsModelError;
use crate::identifiers::{DeadLetterId, JobId, JobQueueName};
use crate::model::{DeadLetterReason, JobInstant};
use crate::runtime::JobSpec;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct SharedJobsRuntimeHarness {
    runtime: Arc<dyn JobsCoordinationRuntime>,
}

impl SharedJobsRuntimeHarness {
    pub(super) fn new(runtime: Arc<dyn JobsCoordinationRuntime>) -> Self {
        Self { runtime }
    }
}

impl JobsCoordinationRuntime for SharedJobsRuntimeHarness {
    fn snapshot(&self) -> crate::JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.runtime.enqueue(spec, now)
    }

    fn retry_dead_letter(
        &self,
        dead_letter_id: &DeadLetterId,
        now: JobInstant,
    ) -> Result<QueuedJobRecord, JobsModelError> {
        self.runtime.retry_dead_letter(dead_letter_id, now)
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        self.runtime
            .acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        self.runtime.promote_due_jobs(node_id, now)
    }

    fn lease_ready_jobs(
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

    fn acknowledge_completed(
        &self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.runtime.acknowledge_completed(lease, now)
    }

    fn acknowledge_failed(
        &self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        self.runtime
            .acknowledge_failed(lease, now, reason, error_message)
    }

    fn cancel(&self, queue: &JobQueueName, job_id: &JobId) -> Result<bool, JobsModelError> {
        self.runtime.cancel(queue, job_id)
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

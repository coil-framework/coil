use super::*;
use std::sync::Mutex;

#[derive(Debug)]
pub(super) struct EmulatedJobsCoordinationRuntime {
    state: Mutex<JobsBackendState>,
}

impl EmulatedJobsCoordinationRuntime {
    pub(super) fn new(runtime: JobsRuntime) -> Self {
        Self {
            state: Mutex::new(JobsBackendState::new(runtime)),
        }
    }
}

impl JobsCoordinationRuntime for EmulatedJobsCoordinationRuntime {
    fn snapshot(&self) -> JobsCoordinatorSnapshot {
        let guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.snapshot()
    }

    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.enqueue(spec, now)
    }

    fn retry_dead_letter(
        &self,
        dead_letter_id: &DeadLetterId,
        now: JobInstant,
    ) -> Result<QueuedJobRecord, JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.retry_dead_letter(dead_letter_id, now)
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.promote_due_jobs(node_id, now)
    }

    fn lease_ready_jobs(
        &self,
        queue: &JobQueueName,
        worker_id: String,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
    }

    fn acknowledge_completed(
        &self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.acknowledge_completed(lease, now)
    }

    fn acknowledge_failed(
        &self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        let mut guard = self.state.lock().expect("jobs backend mutex poisoned");
        guard.acknowledge_failed(lease, now, reason, error_message)
    }

    fn is_shared_backend(&self) -> bool {
        false
    }
}

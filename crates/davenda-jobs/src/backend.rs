use crate::error::JobsModelError;
use crate::identifiers::{DeadLetterId, IdempotencyKey, JobId, JobQueueName};
use crate::model::{DeadLetterOutcome, DeadLetterReason, JobInstant, QueueTopology};
use crate::runtime::{JobSpec, JobsRuntime};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecutionContext {
    pub job_id: JobId,
    pub queue: JobQueueName,
    pub backend: davenda_config::JobBackend,
    pub idempotency_key: Option<IdempotencyKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedJobRecord {
    pub spec: JobSpec,
    pub attempts: u32,
    pub enqueued_at: JobInstant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLease {
    pub record: QueuedJobRecord,
    pub worker_id: String,
    pub leased_at: JobInstant,
    pub lease_until: JobInstant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailureDisposition {
    Retried {
        job_id: JobId,
        next_attempt_at: JobInstant,
        queue: JobQueueName,
    },
    DeadLettered(DeadLetterOutcome),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
}

#[derive(Debug)]
struct EmulatedJobsCoordinationRuntime {
    state: Mutex<JobsBackendState>,
}

impl EmulatedJobsCoordinationRuntime {
    fn new(runtime: JobsRuntime) -> Self {
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

    pub fn emulated_shared_runtime(runtime: &JobsRuntime) -> Arc<dyn JobsCoordinationRuntime> {
        Arc::new(EmulatedJobsCoordinationRuntime::new(runtime.clone()))
    }

    #[doc(hidden)]
    pub fn in_memory(runtime: &JobsRuntime) -> Self {
        Self::local_for_testing(runtime)
    }

    #[doc(hidden)]
    pub fn local_for_testing(runtime: &JobsRuntime) -> Self {
        Self {
            backend: runtime.backend,
            queue_topology: runtime.topology.clone(),
            shared: false,
            runtime: Self::emulated_shared_runtime(runtime),
        }
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(runtime). use with_shared_runtime(backend, topology, runtime) or local_for_testing(runtime)"
    )]
    pub fn shared(runtime: &JobsRuntime) -> Self {
        Self::local_for_testing(runtime)
    }

    #[allow(dead_code)]
    #[doc(hidden)]
    #[deprecated(
        note = "compatibility shim; behaves like local_for_testing(runtime). use with_shared_runtime(backend, topology, runtime) or local_for_testing(runtime)"
    )]
    pub fn shared_scoped(runtime: &JobsRuntime, _scope: impl Into<String>) -> Self {
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

impl fmt::Debug for JobsBackendAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobsBackendAdapter")
            .field("backend", &self.backend)
            .field("queue_topology", &self.queue_topology)
            .finish()
    }
}

#[derive(Debug, Clone)]
struct JobsBackendState {
    runtime: JobsRuntime,
    ready: Vec<QueuedJobRecord>,
    scheduled: Vec<QueuedJobRecord>,
    in_flight: Vec<JobLease>,
    dead_letters: Vec<DeadLetterOutcome>,
    leadership: Option<SchedulerLeadership>,
}

impl JobsBackendState {
    fn new(runtime: JobsRuntime) -> Self {
        Self {
            runtime,
            ready: Vec::new(),
            scheduled: Vec::new(),
            in_flight: Vec::new(),
            dead_letters: Vec::new(),
            leadership: None,
        }
    }

    fn snapshot(&self) -> JobsCoordinatorSnapshot {
        JobsCoordinatorSnapshot {
            ready: self.ready.clone(),
            scheduled: self.scheduled.clone(),
            in_flight: self.in_flight.clone(),
            dead_letters: self.dead_letters.clone(),
            leadership: self.leadership.clone(),
        }
    }

    fn enqueue(&mut self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        let planned = self.runtime.planner().plan_job(spec.clone(), now)?;
        let record = QueuedJobRecord {
            spec: JobSpec {
                queue: planned.queue,
                scheduled_for: planned.scheduled_for,
                retry_policy: planned.retry_policy,
                idempotency_key: planned.idempotency_key,
                ..spec
            },
            attempts: 0,
            enqueued_at: now,
        };

        if record.spec.scheduled_for.is_some() {
            self.scheduled.push(record);
        } else {
            self.ready.push(record);
        }

        Ok(())
    }

    fn acquire_scheduler_leadership(
        &mut self,
        node_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        let node_id = crate::validation::require_non_empty("node_id", node_id.into())?;
        if let Some(current) = self.leadership.as_ref() {
            if current.is_active(now) && current.node_id != node_id {
                return Err(JobsModelError::LeadershipConflict {
                    current_holder: current.node_id.clone(),
                    requested_holder: node_id,
                });
            }
        }

        let leadership = SchedulerLeadership {
            node_id,
            acquired_at: now,
            lease_until: now.checked_add(lease_ttl)?,
        };
        self.leadership = Some(leadership.clone());
        Ok(leadership)
    }

    fn promote_due_jobs(
        &mut self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        self.require_active_leadership(node_id, now)?;

        let mut promoted_ids = Vec::new();
        let mut remaining = Vec::new();
        for mut job in self.scheduled.drain(..) {
            if job
                .spec
                .scheduled_for
                .is_some_and(|scheduled_for| scheduled_for <= now)
            {
                promoted_ids.push(job.spec.job_id.clone());
                job.spec.scheduled_for = None;
                self.ready.push(job);
            } else {
                remaining.push(job);
            }
        }
        self.scheduled = remaining;
        Ok(promoted_ids)
    }

    fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        let worker_id = crate::validation::require_non_empty("worker_id", worker_id.into())?;
        self.runtime
            .topology
            .queue(queue)
            .ok_or_else(|| JobsModelError::UnknownQueue {
                queue: queue.to_string(),
            })?;

        let lease_until = now.checked_add(lease_ttl)?;
        let mut leased = Vec::new();
        let mut remaining = Vec::new();

        for job in self.ready.drain(..) {
            if leased.len() < max_jobs && &job.spec.queue == queue {
                let lease = JobLease {
                    record: job,
                    worker_id: worker_id.clone(),
                    leased_at: now,
                    lease_until,
                };
                self.in_flight.push(lease.clone());
                leased.push(lease);
            } else {
                remaining.push(job);
            }
        }

        self.ready = remaining;
        Ok(leased)
    }

    fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.ensure_active_lease(lease, now)?;
        self.remove_in_flight(&lease.record.spec.job_id)?;
        Ok(())
    }

    fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        self.ensure_active_lease(lease, now)?;
        let error_message =
            crate::validation::require_non_empty("job_error_message", error_message.into())?;
        let mut record = self.remove_in_flight(&lease.record.spec.job_id)?;
        record.attempts += 1;

        if record.attempts < record.spec.retry_policy.max_attempts {
            let delay = record.spec.retry_policy.delay_for_attempt(record.attempts);
            let next_attempt_at = now.checked_add(delay)?;
            if delay.is_zero() {
                record.spec.scheduled_for = None;
                self.ready.push(record.clone());
            } else {
                record.spec.scheduled_for = Some(next_attempt_at);
                self.scheduled.push(record.clone());
            }

            Ok(JobFailureDisposition::Retried {
                job_id: record.spec.job_id,
                next_attempt_at,
                queue: record.spec.queue,
            })
        } else {
            let routed_to = record
                .spec
                .retry_policy
                .dead_letter_queue
                .clone()
                .or_else(|| {
                    self.runtime
                        .topology
                        .queue(&record.spec.queue)
                        .and_then(|queue| queue.dead_letter_queue.clone())
                });
            let outcome = DeadLetterOutcome::new(
                DeadLetterId::new(format!("dead-letter:{}", record.spec.job_id.as_str()))?,
                record.spec.job_id.clone(),
                record.spec.queue.clone(),
                reason,
                record.attempts,
                error_message,
                routed_to,
            )?;
            self.dead_letters.push(outcome.clone());
            Ok(JobFailureDisposition::DeadLettered(outcome))
        }
    }

    fn require_active_leadership(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        match self.leadership.as_ref() {
            Some(leadership) if leadership.node_id == node_id && leadership.is_active(now) => {
                Ok(())
            }
            Some(leadership) if leadership.node_id == node_id => {
                Err(JobsModelError::SchedulerLeadershipExpired {
                    node_id: node_id.to_string(),
                    lease_until: leadership.lease_until,
                    now,
                })
            }
            Some(_) | None => Err(JobsModelError::MissingSchedulerLeadership {
                node_id: node_id.to_string(),
            }),
        }
    }

    fn ensure_active_lease(&self, lease: &JobLease, now: JobInstant) -> Result<(), JobsModelError> {
        if lease.lease_until <= now {
            return Err(JobsModelError::LeaseExpired {
                job_id: lease.record.spec.job_id.to_string(),
                lease_until: lease.lease_until,
                now,
            });
        }

        self.in_flight
            .iter()
            .find(|current| current.record.spec.job_id == lease.record.spec.job_id)
            .ok_or_else(|| JobsModelError::UnknownInFlightJob {
                job_id: lease.record.spec.job_id.to_string(),
            })?;

        Ok(())
    }

    fn remove_in_flight(&mut self, job_id: &JobId) -> Result<QueuedJobRecord, JobsModelError> {
        let index = self
            .in_flight
            .iter()
            .position(|lease| &lease.record.spec.job_id == job_id)
            .ok_or_else(|| JobsModelError::UnknownInFlightJob {
                job_id: job_id.to_string(),
            })?;
        Ok(self.in_flight.remove(index).record)
    }
}

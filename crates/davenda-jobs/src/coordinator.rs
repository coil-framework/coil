use crate::backend::QueuedJobRecord;
use crate::backend::{JobFailureDisposition, JobLease, JobsBackendAdapter, SchedulerLeadership};
use crate::error::JobsModelError;
use crate::events::DomainEventEnvelope;
use crate::identifiers::{JobId, JobName, JobQueueName};
use crate::model::{DeadLetterReason, JobInstant};
use crate::runtime::{JobSpec, JobsRuntime};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct JobsCoordinator {
    backend: JobsBackendAdapter,
    snapshot: crate::JobsCoordinatorSnapshot,
}

impl JobsCoordinator {
    pub fn new(runtime: JobsRuntime) -> Result<Self, JobsModelError> {
        Err(crate::backend::explicit_distributed_backend_error(&runtime))
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub(crate) fn new_in_memory(runtime: JobsRuntime) -> Self {
        Self::new_for_testing(runtime)
    }

    #[cfg(test)]
    pub fn new_for_testing(runtime: JobsRuntime) -> Self {
        let backend = JobsBackendAdapter::local_for_testing(&runtime)
            .expect("test-only local jobs coordinator backend must be available");
        Self::with_backend(runtime, backend)
    }

    pub fn new_with_shared_runtime(
        runtime: JobsRuntime,
        shared_runtime: Arc<dyn crate::JobsCoordinationRuntime>,
    ) -> Self {
        let backend = JobsBackendAdapter::with_shared_runtime(
            runtime.backend,
            runtime.topology.clone(),
            shared_runtime,
        );
        Self::with_backend(runtime, backend)
    }

    pub fn with_backend(runtime: JobsRuntime, backend: JobsBackendAdapter) -> Self {
        let _ = runtime;
        Self {
            snapshot: backend.snapshot(),
            backend,
        }
    }

    pub fn ready_jobs(&self) -> &[QueuedJobRecord] {
        &self.snapshot.ready
    }

    pub fn scheduled_jobs(&self) -> &[QueuedJobRecord] {
        &self.snapshot.scheduled
    }

    pub fn in_flight_jobs(&self) -> &[JobLease] {
        &self.snapshot.in_flight
    }

    pub fn dead_letters(&self) -> &[crate::DeadLetterOutcome] {
        &self.snapshot.dead_letters
    }

    pub fn leadership(&self) -> Option<&SchedulerLeadership> {
        self.snapshot.leadership.as_ref()
    }

    pub fn refresh(&mut self) {
        self.snapshot = self.backend.snapshot();
    }

    pub fn enqueue(&mut self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.backend.enqueue(spec, now)?;
        self.refresh();
        Ok(())
    }

    pub fn acquire_scheduler_leadership(
        &mut self,
        node_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        let leadership =
            self.backend
                .acquire_scheduler_leadership(node_id.into(), now, lease_ttl)?;
        self.refresh();
        Ok(leadership)
    }

    pub fn promote_due_jobs(
        &mut self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        let promoted = self.backend.promote_due_jobs(node_id, now)?;
        self.refresh();
        Ok(promoted)
    }

    pub fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        let leased =
            self.backend
                .lease_ready_jobs(queue, worker_id.into(), now, lease_ttl, max_jobs)?;
        self.refresh();
        Ok(leased)
    }

    pub fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.backend.acknowledge_completed(lease, now)?;
        self.refresh();
        Ok(())
    }

    pub fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        let outcome = self
            .backend
            .acknowledge_failed(lease, now, reason, error_message.into())?;
        self.refresh();
        Ok(outcome)
    }

    pub fn dispatch_event<P>(
        &mut self,
        domain: &crate::JobsDomain,
        event: &DomainEventEnvelope<P>,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        let mut planned = Vec::new();

        for subscription in domain
            .domain_event_subscriptions
            .iter()
            .filter(|subscription| subscription.event_type == event.event_type)
        {
            if !domain
                .handlers
                .iter()
                .any(|handler| handler.id == subscription.handler)
            {
                return Err(JobsModelError::MissingEventHandler {
                    handler_id: subscription.handler.to_string(),
                });
            }

            let spec = JobSpec::new(
                JobId::new(format!(
                    "event:{}:{}",
                    event.event_id.as_str(),
                    subscription.id.as_str()
                ))?,
                JobName::new(format!("event-handler:{}", subscription.handler.as_str()))?,
                subscription.queue.clone(),
                format!(
                    "dispatch {} for {}:{}",
                    event.event_type, event.aggregate_kind, event.aggregate_id
                ),
            )?
            .with_retry_policy(subscription.retry_policy.clone());
            let spec = match subscription.idempotency_key.clone() {
                Some(key) => spec.with_idempotency_key(key),
                None => spec,
            };
            let job_id = spec.job_id.clone();
            self.backend.enqueue(spec, now)?;
            planned.push(job_id);
        }

        self.refresh();
        Ok(planned)
    }
}

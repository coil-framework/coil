use super::super::*;
use super::errors::RuntimeJobsError;
use super::request::{DomainEventDispatchRequest, JobDispatchRequest};
use super::types::{DomainEventDispatch, RuntimeEventSubscriptionDefinition, RuntimeJobDefinition};
use coil_jobs::DeadLetterId;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct JobsHost {
    pub customer_app: String,
    pub scheduler_node_id: String,
    pub runtime: JobsRuntimeServices,
    pub telemetry: coil_observability::TelemetryCatalog,
    pub queue_topology: QueueTopology,
    pub registered_jobs: Vec<RuntimeJobDefinition>,
    pub registered_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    pub shared_backend_namespace: String,
    coordinator: JobsCoordinator,
    next_job_sequence: u64,
    next_event_sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobsQueueSnapshot {
    pub ready: usize,
    pub scheduled: usize,
    pub in_flight: usize,
    pub dead_letters: usize,
}

impl JobsHost {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(
        customer_app: String,
        scheduler_node_id: String,
        runtime: JobsRuntimeServices,
        telemetry: coil_observability::TelemetryCatalog,
        queue_topology: QueueTopology,
        registered_jobs: Vec<RuntimeJobDefinition>,
        registered_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
        jobs_domain: JobsDomain,
        shared_runtime: Arc<dyn coil_jobs::JobsCoordinationRuntime>,
        shared_backend_namespace: String,
    ) -> Self {
        let coordinator = runtime.coordinator_with_shared_runtime(shared_runtime);
        Self {
            customer_app,
            scheduler_node_id,
            runtime,
            telemetry,
            queue_topology,
            registered_jobs,
            registered_event_subscriptions,
            jobs_domain,
            shared_backend_namespace,
            coordinator,
            next_job_sequence: 0,
            next_event_sequence: 0,
        }
    }

    pub fn enqueue_spec(
        &mut self,
        spec: JobSpec,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        self.refresh_observability("jobs.enqueue_spec", "ok", Some(job_id.as_str()), now);
        Ok(job_id)
    }

    pub fn retry_dead_letter(
        &mut self,
        dead_letter_id: impl Into<String>,
        now_unix_seconds: u64,
    ) -> Result<JobId, RuntimeJobsError> {
        let dead_letter_id = DeadLetterId::new(dead_letter_id.into())?;
        let now = JobInstant::from_unix_seconds(now_unix_seconds);
        let record = self.coordinator.retry_dead_letter(&dead_letter_id, now)?;
        self.refresh_observability(
            "jobs.retry_dead_letter",
            "ok",
            Some(record.spec.job_id.as_str()),
            now,
        );
        Ok(record.spec.job_id)
    }

    pub fn enqueue_job(
        &mut self,
        request: JobDispatchRequest,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let Some(definition) = self
            .registered_jobs
            .iter()
            .find(|definition| definition.contract.name == request.job_name)
            .cloned()
        else {
            return Err(RuntimeJobsError::UnknownJob {
                job: request.job_name,
            });
        };

        match definition.contract.trigger {
            JobTriggerKind::Scheduled if request.scheduled_for.is_none() => {
                return Err(RuntimeJobsError::ScheduledJobRequiresSchedule {
                    job: definition.contract.name,
                });
            }
            JobTriggerKind::Scheduled => {}
            JobTriggerKind::DomainEvent => {
                return Err(RuntimeJobsError::DomainEventJobRequiresEventDispatch {
                    job: definition.contract.name,
                });
            }
            trigger if request.scheduled_for.is_some() => {
                return Err(RuntimeJobsError::UnexpectedSchedule {
                    job: definition.contract.name,
                    trigger,
                });
            }
            _ => {}
        }

        let mut spec = JobSpec::new(
            self.issue_job_id(&definition.contract.name)?,
            JobName::new(definition.contract.name.clone())?,
            definition.queue.clone(),
            request.payload_description,
        )?
        .with_retry_policy(definition.retry_policy.clone());

        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }

        match request.idempotency_key {
            Some(key) => {
                spec = spec.with_idempotency_key(IdempotencyKey::new(key)?);
            }
            None if definition.retry_policy.is_retrying() => {
                return Err(RuntimeJobsError::MissingIdempotencyKey {
                    job: definition.contract.name,
                });
            }
            None => {}
        }

        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        self.refresh_observability("jobs.enqueue_job", "ok", Some(job_id.as_str()), now);
        Ok(job_id)
    }

    pub fn emit_domain_event(
        &mut self,
        request: DomainEventDispatchRequest,
        now: JobInstant,
    ) -> Result<DomainEventDispatch, RuntimeJobsError> {
        let event_type = DomainEventType::new(request.event_type.clone())?;
        let event_id = self.issue_event_id(&request.event_type)?;
        let mut envelope = DomainEventEnvelope::new(
            event_id.clone(),
            event_type.clone(),
            request.aggregate_kind,
            request.aggregate_id,
            now,
            request.payload_description,
        )?;

        if let Some(correlation_id) = request.correlation_id {
            envelope = envelope.with_correlation_id(correlation_id)?;
        }

        if let Some(causation_id) = request.causation_id {
            envelope = envelope.with_causation_id(causation_id)?;
        }

        let mut enqueued_jobs = Vec::new();
        for subscription in self
            .registered_event_subscriptions
            .iter()
            .filter(|subscription| subscription.event_type == event_type)
            .cloned()
        {
            let mut spec = JobSpec::new(
                JobId::new(format!(
                    "event:{}:{}",
                    event_id.as_str(),
                    subscription.subscription_id.as_str()
                ))?,
                JobName::new(format!("event-handler:{}", subscription.job_name))?,
                subscription.reaction_queue,
                format!(
                    "dispatch {} for {}:{}",
                    event_type.as_str(),
                    envelope.aggregate_kind,
                    envelope.aggregate_id
                ),
            )?
            .with_retry_policy(subscription.retry_policy.clone());

            if subscription.retry_policy.is_retrying() {
                spec = spec.with_idempotency_key(IdempotencyKey::new(format!(
                    "event:{}:{}:{}",
                    event_id.as_str(),
                    subscription.module,
                    subscription.job_name
                ))?);
            }

            let job_id = spec.job_id.clone();
            self.coordinator.enqueue(spec, now)?;
            enqueued_jobs.push(job_id);
        }

        self.refresh_observability("jobs.emit_domain_event", "ok", Some(event_id.as_str()), now);

        Ok(DomainEventDispatch {
            event_id,
            event_type,
            enqueued_jobs,
        })
    }

    pub fn acquire_scheduler_leadership(
        &mut self,
        now: JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<SchedulerLeadership, RuntimeJobsError> {
        let leadership = self.coordinator.acquire_scheduler_leadership(
            self.scheduler_node_id.clone(),
            now,
            lease_ttl,
        )?;
        self.refresh_observability("jobs.acquire_scheduler_leadership", "ok", None, now);
        Ok(leadership)
    }

    pub fn promote_due_jobs(&mut self, now: JobInstant) -> Result<Vec<JobId>, RuntimeJobsError> {
        let promoted = self
            .coordinator
            .promote_due_jobs(&self.scheduler_node_id, now)?;
        self.refresh_observability("jobs.promote_due_jobs", "ok", None, now);
        Ok(promoted)
    }

    pub fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, RuntimeJobsError> {
        let leased = self
            .coordinator
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)?;
        self.refresh_observability("jobs.lease_ready_jobs", "ok", None, now);
        Ok(leased)
    }

    pub fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), RuntimeJobsError> {
        self.coordinator.acknowledge_completed(lease, now)?;
        self.refresh_observability(
            "jobs.acknowledge_completed",
            "ok",
            Some(lease.record.spec.job_id.as_str()),
            now,
        );
        Ok(())
    }

    pub fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, RuntimeJobsError> {
        let outcome = self
            .coordinator
            .acknowledge_failed(lease, now, reason, error_message.into())?;
        self.refresh_observability(
            "jobs.acknowledge_failed",
            "ok",
            Some(lease.record.spec.job_id.as_str()),
            now,
        );
        Ok(outcome)
    }

    pub fn coordinator(&self) -> &JobsCoordinator {
        &self.coordinator
    }

    pub fn queue_snapshot(&mut self) -> JobsQueueSnapshot {
        self.coordinator.refresh();
        JobsQueueSnapshot {
            ready: self.coordinator.ready_jobs().len(),
            scheduled: self.coordinator.scheduled_jobs().len(),
            in_flight: self.coordinator.in_flight_jobs().len(),
            dead_letters: self.coordinator.dead_letters().len(),
        }
    }

    fn issue_job_id(&mut self, job_name: &str) -> Result<JobId, RuntimeJobsError> {
        self.next_job_sequence += 1;
        Ok(JobId::new(format!(
            "job:{}:{}",
            job_name, self.next_job_sequence
        ))?)
    }

    fn issue_event_id(&mut self, event_type: &str) -> Result<DomainEventId, RuntimeJobsError> {
        self.next_event_sequence += 1;
        Ok(DomainEventId::new(format!(
            "evt:{}:{}",
            event_type, self.next_event_sequence
        ))?)
    }

    fn refresh_observability(
        &mut self,
        span: &str,
        outcome: &str,
        trace_id: Option<&str>,
        now: JobInstant,
    ) {
        let snapshot = self.queue_snapshot();
        let depth = snapshot.ready + snapshot.scheduled + snapshot.in_flight;
        let _ = self
            .telemetry
            .set_gauge("coil.queue.depth", depth.min(i64::MAX as usize) as i64);
        let _ = self.telemetry.record_trace(
            coil_observability::TraceRecord::new(
                trace_id
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| format!("{}:{}", span, self.scheduler_node_id)),
                span,
                outcome,
                now.as_unix_seconds(),
            )
            .with_field("customer_app", self.customer_app.clone())
            .with_field("scheduler_node_id", self.scheduler_node_id.clone())
            .with_field("ready", snapshot.ready.to_string())
            .with_field("scheduled", snapshot.scheduled.to_string())
            .with_field("in_flight", snapshot.in_flight.to_string())
            .with_field("dead_letters", snapshot.dead_letters.to_string()),
        );
    }
}

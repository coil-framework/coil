use crate::backend::JobsBackendAdapter;
use crate::coordinator::JobsCoordinator;
use crate::error::JobsModelError;
use crate::identifiers::{IdempotencyKey, JobId, JobName, JobQueueName};
use crate::model::{JobInstant, QueueTopology, RetryPolicy};
use crate::validation::require_non_empty;
use davenda_config::{JobBackend, JobsConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct JobsRuntime {
    pub backend: JobBackend,
    pub topology: QueueTopology,
    pub default_retry_limit: u32,
}

impl JobsRuntime {
    pub fn from_config(config: &JobsConfig) -> Result<Self, JobsModelError> {
        let work_queue = crate::JobQueueName::new("jobs.work")?;
        let scheduled_queue = crate::JobQueueName::new("jobs.scheduled")?;
        let domain_events_queue = crate::JobQueueName::new("jobs.domain-events")?;
        let dead_letter_queue = crate::JobQueueName::new("jobs.dead-letter")?;

        let default_retry_policy = RetryPolicy::new(
            config.retry_limit.max(1),
            Duration::from_secs(5),
            Duration::from_secs(300),
        )?
        .with_dead_letter_queue(dead_letter_queue.clone());

        let queues = vec![
            crate::QueueDefinition::new(
                work_queue.clone(),
                crate::QueueKind::Work,
                16,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            crate::QueueDefinition::new(
                scheduled_queue.clone(),
                crate::QueueKind::Scheduled,
                4,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            crate::QueueDefinition::new(
                domain_events_queue.clone(),
                crate::QueueKind::DomainEvents,
                8,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            crate::QueueDefinition::new(
                dead_letter_queue.clone(),
                crate::QueueKind::DeadLetter,
                1,
                RetryPolicy::default(),
            )?,
        ];

        let topology = QueueTopology {
            backend: config.backend,
            work_queue,
            scheduled_queue,
            domain_events_queue,
            dead_letter_queue,
            queues,
            scheduler_lock_key: "jobs:scheduler:leader".to_string(),
        };
        topology.validate()?;

        Ok(Self {
            backend: config.backend,
            topology,
            default_retry_limit: config.retry_limit.max(1),
        })
    }

    pub fn describe(&self) -> &QueueTopology {
        &self.topology
    }

    pub fn planner(&self) -> crate::JobsPlanner {
        crate::JobsPlanner::new(self.clone())
    }

    pub fn coordinator(&self) -> Result<JobsCoordinator, JobsModelError> {
        Err(crate::backend::explicit_distributed_backend_error(self))
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub(crate) fn coordinator_in_memory(&self) -> JobsCoordinator {
        self.coordinator_for_testing()
    }

    #[doc(hidden)]
    #[cfg(test)]
    pub fn coordinator_for_testing(&self) -> JobsCoordinator {
        let backend = JobsBackendAdapter::local_for_testing(self)
            .expect("test-only local jobs coordinator backend must be available");
        self.coordinator_with_backend(backend)
    }

    pub fn coordinator_with_shared_runtime(
        &self,
        runtime: Arc<dyn crate::JobsCoordinationRuntime>,
    ) -> JobsCoordinator {
        self.coordinator_with_backend(JobsBackendAdapter::with_shared_runtime(
            self.backend,
            self.topology.clone(),
            runtime,
        ))
    }

    pub fn coordinator_with_backend(&self, backend: JobsBackendAdapter) -> JobsCoordinator {
        JobsCoordinator::with_backend(self.clone(), backend)
    }
}

impl PartialEq for JobsRuntime {
    fn eq(&self, other: &Self) -> bool {
        self.backend == other.backend
            && self.topology == other.topology
            && self.default_retry_limit == other.default_retry_limit
    }
}

impl Eq for JobsRuntime {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadLetterOutcomeKind {
    RouteToQueue(JobQueueName),
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSpec {
    pub job_id: JobId,
    pub job_name: JobName,
    pub queue: JobQueueName,
    pub scheduled_for: Option<JobInstant>,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<IdempotencyKey>,
    pub payload_description: String,
}

impl JobSpec {
    pub fn new(
        job_id: JobId,
        job_name: JobName,
        queue: JobQueueName,
        payload_description: impl Into<String>,
    ) -> Result<Self, JobsModelError> {
        Ok(Self {
            job_id,
            job_name,
            queue,
            scheduled_for: None,
            retry_policy: RetryPolicy::default(),
            idempotency_key: None,
            payload_description: require_non_empty(
                "payload_description",
                payload_description.into(),
            )?,
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedJob {
    pub job_id: JobId,
    pub job_name: JobName,
    pub queue: JobQueueName,
    pub scheduled_for: Option<JobInstant>,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<IdempotencyKey>,
    pub dead_letter_outcome: DeadLetterOutcomeKind,
}

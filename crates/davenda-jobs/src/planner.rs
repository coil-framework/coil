use crate::error::JobsModelError;
use crate::model::{JobInstant, QueueKind};
use crate::runtime::{DeadLetterOutcomeKind, JobSpec, JobsRuntime, PlannedJob};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsPlanner {
    runtime: JobsRuntime,
}

impl JobsPlanner {
    pub fn new(runtime: JobsRuntime) -> Self {
        Self { runtime }
    }

    pub fn runtime(&self) -> &JobsRuntime {
        &self.runtime
    }

    pub fn describe_queue_topology(&self) -> &crate::QueueTopology {
        self.runtime.describe()
    }

    pub fn plan_job(&self, spec: JobSpec, now: JobInstant) -> Result<PlannedJob, JobsModelError> {
        let queue = self.runtime.topology.queue(&spec.queue).ok_or_else(|| {
            JobsModelError::UnknownQueue {
                queue: spec.queue.to_string(),
            }
        })?;

        if let Some(scheduled_for) = spec.scheduled_for {
            if scheduled_for <= now {
                return Err(JobsModelError::ScheduledInPast {
                    scheduled_at: scheduled_for,
                    now,
                });
            }

            if queue.kind != QueueKind::Scheduled {
                return Err(JobsModelError::QueueKindMismatch {
                    queue: queue.name.to_string(),
                    expected: QueueKind::Scheduled,
                    actual: queue.kind,
                });
            }
        }

        self.validate_retry_safety(&spec)?;

        let dead_letter_queue = spec
            .retry_policy
            .dead_letter_queue
            .clone()
            .or_else(|| queue.dead_letter_queue.clone())
            .ok_or_else(|| JobsModelError::MissingDeadLetterQueue {
                queue: queue.name.to_string(),
            })?;

        Ok(PlannedJob {
            job_id: spec.job_id,
            job_name: spec.job_name,
            queue: queue.name.clone(),
            scheduled_for: spec.scheduled_for,
            retry_policy: spec.retry_policy,
            idempotency_key: spec.idempotency_key,
            dead_letter_outcome: DeadLetterOutcomeKind::RouteToQueue(dead_letter_queue),
        })
    }

    pub fn validate_retry_safety(&self, spec: &JobSpec) -> Result<(), JobsModelError> {
        if spec.retry_policy.is_retrying() && spec.idempotency_key.is_none() {
            return Err(JobsModelError::MissingIdempotencyKey {
                job_id: spec.job_id.to_string(),
            });
        }

        Ok(())
    }

    pub fn validate_queue_topology(&self) -> Result<(), JobsModelError> {
        self.runtime.topology.validate()
    }
}

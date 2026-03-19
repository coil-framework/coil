use crate::error::JobsModelError;
use crate::identifiers::{DeadLetterId, JobQueueName};
use crate::validation::require_non_empty;
use davenda_config::JobBackend;
use std::collections::BTreeSet;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobInstant(u64);

impl JobInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, duration: Duration) -> Result<Self, JobsModelError> {
        let added = self.0.checked_add(duration.as_secs()).ok_or_else(|| {
            JobsModelError::InvalidRetryPolicy {
                reason: "job instant overflow".to_string(),
            }
        })?;
        Ok(Self(added))
    }
}

impl fmt::Display for JobInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueKind {
    Work,
    Scheduled,
    DomainEvents,
    DeadLetter,
}

impl fmt::Display for QueueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Work => f.write_str("work"),
            Self::Scheduled => f.write_str("scheduled"),
            Self::DomainEvents => f.write_str("domain_events"),
            Self::DeadLetter => f.write_str("dead_letter"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackoffStrategy {
    Fixed,
    Exponential,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff: BackoffStrategy,
    pub dead_letter_queue: Option<JobQueueName>,
}

impl RetryPolicy {
    pub fn new(
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, JobsModelError> {
        if max_attempts == 0 {
            return Err(JobsModelError::InvalidRetryPolicy {
                reason: "max_attempts must be greater than zero".to_string(),
            });
        }

        if max_delay < initial_delay {
            return Err(JobsModelError::InvalidRetryPolicy {
                reason: "max_delay cannot be shorter than initial_delay".to_string(),
            });
        }

        Ok(Self {
            max_attempts,
            initial_delay,
            max_delay,
            backoff: BackoffStrategy::Exponential,
            dead_letter_queue: None,
        })
    }

    pub fn without_backoff(max_attempts: u32) -> Result<Self, JobsModelError> {
        let mut policy = Self::new(max_attempts, Duration::from_secs(0), Duration::from_secs(0))?;
        policy.backoff = BackoffStrategy::Fixed;
        Ok(policy)
    }

    pub fn with_dead_letter_queue(mut self, queue: JobQueueName) -> Self {
        self.dead_letter_queue = Some(queue);
        self
    }

    pub fn is_retrying(&self) -> bool {
        self.max_attempts > 1
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt <= 1 || self.initial_delay.is_zero() {
            return self.initial_delay;
        }

        match self.backoff {
            BackoffStrategy::Fixed => self.initial_delay,
            BackoffStrategy::Exponential => {
                let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
                let delay = self
                    .initial_delay
                    .checked_mul(factor)
                    .unwrap_or(self.max_delay);
                delay.min(self.max_delay)
            }
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            initial_delay: Duration::from_secs(0),
            max_delay: Duration::from_secs(0),
            backoff: BackoffStrategy::Fixed,
            dead_letter_queue: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadLetterReason {
    ExhaustedRetries,
    DeserializationFailure,
    HandlerPanic,
    IdempotencyConflict,
    PolicyViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetterOutcome {
    pub dead_letter_id: DeadLetterId,
    pub job_id: crate::identifiers::JobId,
    pub queue: JobQueueName,
    pub reason: DeadLetterReason,
    pub failed_attempts: u32,
    pub error_message: String,
    pub routed_to: Option<JobQueueName>,
}

impl DeadLetterOutcome {
    pub fn new(
        dead_letter_id: DeadLetterId,
        job_id: crate::identifiers::JobId,
        queue: JobQueueName,
        reason: DeadLetterReason,
        failed_attempts: u32,
        error_message: impl Into<String>,
        routed_to: Option<JobQueueName>,
    ) -> Result<Self, JobsModelError> {
        Ok(Self {
            dead_letter_id,
            job_id,
            queue,
            reason,
            failed_attempts,
            error_message: require_non_empty("dead_letter_error_message", error_message.into())?,
            routed_to,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueDefinition {
    pub name: JobQueueName,
    pub kind: QueueKind,
    pub concurrency_limit: u16,
    pub retry_policy: RetryPolicy,
    pub dead_letter_queue: Option<JobQueueName>,
}

impl QueueDefinition {
    pub fn new(
        name: JobQueueName,
        kind: QueueKind,
        concurrency_limit: u16,
        retry_policy: RetryPolicy,
    ) -> Result<Self, JobsModelError> {
        if concurrency_limit == 0 {
            return Err(JobsModelError::InvalidRetryPolicy {
                reason: "queue concurrency must be greater than zero".to_string(),
            });
        }

        Ok(Self {
            name,
            kind,
            concurrency_limit,
            retry_policy,
            dead_letter_queue: None,
        })
    }

    pub fn with_dead_letter_queue(mut self, queue: JobQueueName) -> Self {
        self.dead_letter_queue = Some(queue);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueTopology {
    pub backend: JobBackend,
    pub work_queue: JobQueueName,
    pub scheduled_queue: JobQueueName,
    pub domain_events_queue: JobQueueName,
    pub dead_letter_queue: JobQueueName,
    pub queues: Vec<QueueDefinition>,
    pub scheduler_lock_key: String,
}

impl QueueTopology {
    pub fn describe(&self) -> Vec<(String, QueueKind)> {
        self.queues
            .iter()
            .map(|queue| (queue.name.to_string(), queue.kind))
            .collect()
    }

    pub fn queue(&self, name: &JobQueueName) -> Option<&QueueDefinition> {
        self.queues.iter().find(|queue| &queue.name == name)
    }

    pub fn validate(&self) -> Result<(), JobsModelError> {
        let mut seen = BTreeSet::new();
        for queue in &self.queues {
            if !seen.insert(queue.name.as_str()) {
                return Err(JobsModelError::DuplicateIdentifier {
                    kind: "queue",
                    id: queue.name.to_string(),
                });
            }
        }

        for required in [
            &self.work_queue,
            &self.scheduled_queue,
            &self.domain_events_queue,
            &self.dead_letter_queue,
        ] {
            if self.queue(required).is_none() {
                return Err(JobsModelError::UnknownQueue {
                    queue: required.to_string(),
                });
            }
        }

        Ok(())
    }
}

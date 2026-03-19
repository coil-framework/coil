use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_config::{JobBackend, JobsConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobsModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateIdentifier {
        kind: &'static str,
        id: String,
    },
    UnknownQueue {
        queue: String,
    },
    QueueKindMismatch {
        queue: String,
        expected: QueueKind,
        actual: QueueKind,
    },
    MissingIdempotencyKey {
        job_id: String,
    },
    InvalidRetryPolicy {
        reason: String,
    },
    ScheduledInPast {
        scheduled_at: JobInstant,
        now: JobInstant,
    },
    MissingDeadLetterQueue {
        queue: String,
    },
}

impl fmt::Display for JobsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => write!(f, "{kind} `{id}` is duplicated"),
            Self::UnknownQueue { queue } => write!(f, "queue `{queue}` is not registered"),
            Self::QueueKindMismatch {
                queue,
                expected,
                actual,
            } => write!(
                f,
                "queue `{queue}` has kind `{actual}` but `{expected}` was required"
            ),
            Self::MissingIdempotencyKey { job_id } => {
                write!(f, "job `{job_id}` requires an idempotency key for retries")
            }
            Self::InvalidRetryPolicy { reason } => write!(f, "invalid retry policy: {reason}"),
            Self::ScheduledInPast { scheduled_at, now } => {
                write!(
                    f,
                    "scheduled time `{scheduled_at}` must be later than `{now}`"
                )
            }
            Self::MissingDeadLetterQueue { queue } => {
                write!(f, "queue `{queue}` requires a dead-letter queue")
            }
        }
    }
}

impl Error for JobsModelError {}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, JobsModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(JobId, "job_id");
token_type!(JobName, "job_name");
token_type!(JobQueueName, "queue_name");
token_type!(IdempotencyKey, "idempotency_key");
token_type!(ScheduledJobId, "scheduled_job_id");
token_type!(DeadLetterId, "dead_letter_id");
token_type!(DomainEventType, "domain_event_type");
token_type!(DomainEventId, "domain_event_id");
token_type!(EventHandlerId, "event_handler_id");
token_type!(EventSubscriptionId, "event_subscription_id");

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
    pub job_id: JobId,
    pub queue: JobQueueName,
    pub reason: DeadLetterReason,
    pub failed_attempts: u32,
    pub error_message: String,
    pub routed_to: Option<JobQueueName>,
}

impl DeadLetterOutcome {
    pub fn new(
        dead_letter_id: DeadLetterId,
        job_id: JobId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsRuntime {
    pub backend: JobBackend,
    pub topology: QueueTopology,
    pub default_retry_limit: u32,
}

impl JobsRuntime {
    pub fn from_config(config: &JobsConfig) -> Result<Self, JobsModelError> {
        let work_queue = JobQueueName::new("jobs.work")?;
        let scheduled_queue = JobQueueName::new("jobs.scheduled")?;
        let domain_events_queue = JobQueueName::new("jobs.domain-events")?;
        let dead_letter_queue = JobQueueName::new("jobs.dead-letter")?;

        let default_retry_policy = RetryPolicy::new(
            config.retry_limit.max(1),
            Duration::from_secs(5),
            Duration::from_secs(300),
        )?
        .with_dead_letter_queue(dead_letter_queue.clone());

        let queues = vec![
            QueueDefinition::new(
                work_queue.clone(),
                QueueKind::Work,
                16,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            QueueDefinition::new(
                scheduled_queue.clone(),
                QueueKind::Scheduled,
                4,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            QueueDefinition::new(
                domain_events_queue.clone(),
                QueueKind::DomainEvents,
                8,
                default_retry_policy.clone(),
            )?
            .with_dead_letter_queue(dead_letter_queue.clone()),
            QueueDefinition::new(
                dead_letter_queue.clone(),
                QueueKind::DeadLetter,
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

    pub fn planner(&self) -> JobsPlanner {
        JobsPlanner::new(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedJob {
    pub job_id: JobId,
    pub job_name: JobName,
    pub queue: JobQueueName,
    pub scheduled_for: Option<JobInstant>,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<IdempotencyKey>,
    pub dead_letter_outcome: DeadLetterOutcomeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadLetterOutcomeKind {
    RouteToQueue(JobQueueName),
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn describe_queue_topology(&self) -> &QueueTopology {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventEnvelope<P> {
    pub event_id: DomainEventId,
    pub event_type: DomainEventType,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub occurred_at: JobInstant,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub version: u32,
    pub payload: P,
}

impl<P> DomainEventEnvelope<P> {
    pub fn new(
        event_id: DomainEventId,
        event_type: DomainEventType,
        aggregate_kind: impl Into<String>,
        aggregate_id: impl Into<String>,
        occurred_at: JobInstant,
        payload: P,
    ) -> Result<Self, JobsModelError> {
        Ok(Self {
            event_id,
            event_type,
            aggregate_kind: require_non_empty("aggregate_kind", aggregate_kind.into())?,
            aggregate_id: require_non_empty("aggregate_id", aggregate_id.into())?,
            occurred_at,
            correlation_id: None,
            causation_id: None,
            version: 1,
            payload,
        })
    }

    pub fn with_correlation_id(
        mut self,
        correlation_id: impl Into<String>,
    ) -> Result<Self, JobsModelError> {
        self.correlation_id = Some(require_non_empty("correlation_id", correlation_id.into())?);
        Ok(self)
    }

    pub fn with_causation_id(
        mut self,
        causation_id: impl Into<String>,
    ) -> Result<Self, JobsModelError> {
        self.causation_id = Some(require_non_empty("causation_id", causation_id.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSubscriptionMetadata {
    pub id: EventSubscriptionId,
    pub event_type: DomainEventType,
    pub queue: JobQueueName,
    pub handler: EventHandlerId,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<IdempotencyKey>,
    pub description: Option<String>,
}

impl EventSubscriptionMetadata {
    pub fn new(
        id: EventSubscriptionId,
        event_type: DomainEventType,
        queue: JobQueueName,
        handler: EventHandlerId,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            id,
            event_type,
            queue,
            handler,
            retry_policy,
            idempotency_key: None,
            description: None,
        }
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, JobsModelError> {
        self.description = Some(require_non_empty(
            "subscription_description",
            description.into(),
        )?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandlerMetadata {
    pub id: EventHandlerId,
    pub name: String,
    pub queue: JobQueueName,
    pub subscriptions: Vec<EventSubscriptionMetadata>,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<IdempotencyKey>,
}

impl EventHandlerMetadata {
    pub fn new(
        id: EventHandlerId,
        name: impl Into<String>,
        queue: JobQueueName,
        retry_policy: RetryPolicy,
    ) -> Result<Self, JobsModelError> {
        Ok(Self {
            id,
            name: require_non_empty("handler_name", name.into())?,
            queue,
            subscriptions: Vec::new(),
            retry_policy,
            idempotency_key: None,
        })
    }

    pub fn add_subscription(mut self, subscription: EventSubscriptionMetadata) -> Self {
        self.subscriptions.push(subscription);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecutionContext {
    pub job_id: JobId,
    pub queue: JobQueueName,
    pub backend: JobBackend,
    pub idempotency_key: Option<IdempotencyKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsDomain {
    pub runtime: JobsRuntime,
    pub domain_event_subscriptions: Vec<EventSubscriptionMetadata>,
    pub handlers: Vec<EventHandlerMetadata>,
}

impl JobsDomain {
    pub fn new(runtime: JobsRuntime) -> Self {
        Self {
            runtime,
            domain_event_subscriptions: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub fn add_subscription(mut self, subscription: EventSubscriptionMetadata) -> Self {
        self.domain_event_subscriptions.push(subscription);
        self
    }

    pub fn add_handler(mut self, handler: EventHandlerMetadata) -> Self {
        self.handlers.push(handler);
        self
    }

    pub fn validate(&self) -> Result<(), JobsModelError> {
        self.runtime.topology.validate()?;

        let mut seen = BTreeSet::new();
        for subscription in &self.domain_event_subscriptions {
            if !seen.insert(subscription.id.as_str()) {
                return Err(JobsModelError::DuplicateIdentifier {
                    kind: "event subscription",
                    id: subscription.id.to_string(),
                });
            }
        }

        for subscription in &self.domain_event_subscriptions {
            self.runtime
                .topology
                .queue(&subscription.queue)
                .ok_or_else(|| JobsModelError::UnknownQueue {
                    queue: subscription.queue.to_string(),
                })?;

            if subscription.retry_policy.is_retrying() && subscription.idempotency_key.is_none() {
                return Err(JobsModelError::MissingIdempotencyKey {
                    job_id: subscription.id.to_string(),
                });
            }
        }

        for handler in &self.handlers {
            self.runtime.topology.queue(&handler.queue).ok_or_else(|| {
                JobsModelError::UnknownQueue {
                    queue: handler.queue.to_string(),
                }
            })?;
        }

        Ok(())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, JobsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(JobsModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(JobsModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, JobsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(JobsModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(backend: JobBackend) -> JobsConfig {
        JobsConfig {
            backend,
            retry_limit: 3,
        }
    }

    #[test]
    fn runtime_describes_backend_and_queue_topology() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let topology = runtime.describe();

        assert_eq!(runtime.backend, JobBackend::Redis);
        assert_eq!(topology.backend, JobBackend::Redis);
        assert_eq!(topology.work_queue.as_str(), "jobs.work");
        assert_eq!(topology.scheduled_queue.as_str(), "jobs.scheduled");
        assert_eq!(topology.domain_events_queue.as_str(), "jobs.domain-events");
        assert_eq!(topology.dead_letter_queue.as_str(), "jobs.dead-letter");
    }

    #[test]
    fn planner_rejects_retrying_jobs_without_idempotency_keys() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
        let planner = runtime.planner();
        let spec = JobSpec::new(
            JobId::new("job-1").unwrap(),
            JobName::new("reconcile-invoice").unwrap(),
            JobQueueName::new("jobs.work").unwrap(),
            "reconcile invoice state",
        )
        .unwrap()
        .with_retry_policy(
            RetryPolicy::new(3, Duration::from_secs(5), Duration::from_secs(60)).unwrap(),
        );

        let err = planner.validate_retry_safety(&spec).unwrap_err();
        assert!(matches!(
            err,
            JobsModelError::MissingIdempotencyKey { job_id } if job_id == "job-1"
        ));
    }

    #[test]
    fn planner_accepts_scheduled_retry_safe_jobs() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let planner = runtime.planner();
        let spec = JobSpec::new(
            JobId::new("job-2").unwrap(),
            JobName::new("sitemap-regeneration").unwrap(),
            JobQueueName::new("jobs.scheduled").unwrap(),
            "regenerate sitemaps",
        )
        .unwrap()
        .scheduled_for(JobInstant::from_unix_seconds(10))
        .with_retry_policy(
            RetryPolicy::new(4, Duration::from_secs(5), Duration::from_secs(30))
                .unwrap()
                .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
        )
        .with_idempotency_key(IdempotencyKey::new("sitemap:v1").unwrap());

        let planned = planner
            .plan_job(spec, JobInstant::from_unix_seconds(5))
            .unwrap();
        assert_eq!(planned.queue.as_str(), "jobs.scheduled");
        assert!(matches!(
            planned.dead_letter_outcome,
            DeadLetterOutcomeKind::RouteToQueue(ref queue) if queue.as_str() == "jobs.dead-letter"
        ));
    }

    #[test]
    fn planner_rejects_scheduled_jobs_in_the_past() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let planner = runtime.planner();
        let spec = JobSpec::new(
            JobId::new("job-3").unwrap(),
            JobName::new("cache-warm").unwrap(),
            JobQueueName::new("jobs.scheduled").unwrap(),
            "warm cache",
        )
        .unwrap()
        .scheduled_for(JobInstant::from_unix_seconds(5))
        .with_idempotency_key(IdempotencyKey::new("cache:warm:v1").unwrap());

        let err = planner
            .plan_job(spec, JobInstant::from_unix_seconds(5))
            .unwrap_err();
        assert!(matches!(
            err,
            JobsModelError::ScheduledInPast { scheduled_at, now }
                if scheduled_at.as_unix_seconds() == 5 && now.as_unix_seconds() == 5
        ));
    }

    #[test]
    fn domain_event_metadata_tracks_subscriptions_and_handlers() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
        let subscription = EventSubscriptionMetadata::new(
            EventSubscriptionId::new("sub-1").unwrap(),
            DomainEventType::new("order.completed").unwrap(),
            runtime.describe().domain_events_queue.clone(),
            EventHandlerId::new("handler-1").unwrap(),
            RetryPolicy::new(2, Duration::from_secs(10), Duration::from_secs(120))
                .unwrap()
                .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
        )
        .with_idempotency_key(IdempotencyKey::new("event:order.completed:v1").unwrap())
        .with_description("enqueue fulfillment work")
        .unwrap();

        let handler = EventHandlerMetadata::new(
            EventHandlerId::new("handler-1").unwrap(),
            "Fulfillment handler",
            runtime.describe().domain_events_queue.clone(),
            RetryPolicy::default(),
        )
        .unwrap()
        .add_subscription(subscription.clone());

        let domain = JobsDomain::new(runtime)
            .add_subscription(subscription)
            .add_handler(handler);

        assert!(domain.validate().is_ok());
    }

    #[test]
    fn domain_event_envelopes_capture_correlation_and_causation() {
        let envelope = DomainEventEnvelope::new(
            DomainEventId::new("evt-1").unwrap(),
            DomainEventType::new("booking.created").unwrap(),
            "booking",
            "booking-42",
            JobInstant::from_unix_seconds(100),
            "payload".to_string(),
        )
        .unwrap()
        .with_correlation_id("corr-1")
        .unwrap()
        .with_causation_id("cause-1")
        .unwrap();

        assert_eq!(envelope.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(envelope.causation_id.as_deref(), Some("cause-1"));
        assert_eq!(envelope.version, 1);
    }
}

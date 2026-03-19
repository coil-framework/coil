use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
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
    LeadershipConflict {
        current_holder: String,
        requested_holder: String,
    },
    MissingSchedulerLeadership {
        node_id: String,
    },
    SchedulerLeadershipExpired {
        node_id: String,
        lease_until: JobInstant,
        now: JobInstant,
    },
    UnknownInFlightJob {
        job_id: String,
    },
    LeaseExpired {
        job_id: String,
        lease_until: JobInstant,
        now: JobInstant,
    },
    MissingEventHandler {
        handler_id: String,
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
            Self::LeadershipConflict {
                current_holder,
                requested_holder,
            } => write!(
                f,
                "scheduler leadership is held by `{current_holder}`, `{requested_holder}` cannot take it"
            ),
            Self::MissingSchedulerLeadership { node_id } => {
                write!(f, "node `{node_id}` does not hold scheduler leadership")
            }
            Self::SchedulerLeadershipExpired {
                node_id,
                lease_until,
                now,
            } => write!(
                f,
                "scheduler leadership for `{node_id}` expired at `{lease_until}`, current time is `{now}`"
            ),
            Self::UnknownInFlightJob { job_id } => {
                write!(f, "job `{job_id}` is not currently leased")
            }
            Self::LeaseExpired {
                job_id,
                lease_until,
                now,
            } => write!(
                f,
                "lease for job `{job_id}` expired at `{lease_until}`, current time is `{now}`"
            ),
            Self::MissingEventHandler { handler_id } => {
                write!(f, "event handler `{handler_id}` is not registered")
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

#[derive(Debug, Clone)]
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

    pub fn coordinator(&self) -> JobsCoordinator {
        self.coordinator_with_backend(JobsBackendAdapter::in_memory(self))
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
}

#[derive(Clone)]
pub struct JobsBackendAdapter {
    backend: JobBackend,
    queue_topology: QueueTopology,
    runtime: Arc<dyn JobsCoordinationRuntime>,
}

impl JobsBackendAdapter {
    pub fn new(
        backend: JobBackend,
        queue_topology: QueueTopology,
        runtime: Arc<dyn JobsCoordinationRuntime>,
    ) -> Self {
        Self {
            backend,
            queue_topology,
            runtime,
        }
    }

    pub fn in_memory(runtime: &JobsRuntime) -> Self {
        Self::new(
            runtime.backend,
            runtime.topology.clone(),
            Arc::new(EmulatedJobsCoordinationRuntime::new(runtime.clone())),
        )
    }

    pub fn shared(runtime: &JobsRuntime) -> Self {
        Self::shared_scoped(runtime, format!("{:p}", runtime))
    }

    pub fn shared_scoped(runtime: &JobsRuntime, scope: impl Into<String>) -> Self {
        Self::new(
            runtime.backend,
            runtime.topology.clone(),
            shared_jobs_runtime(runtime, scope.into()),
        )
    }

    fn snapshot(&self) -> JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.runtime.enqueue(spec, now)
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
}

fn shared_jobs_runtime(
    runtime: &JobsRuntime,
    scope: String,
) -> Arc<dyn JobsCoordinationRuntime> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, Arc<dyn JobsCoordinationRuntime>>>> =
        OnceLock::new();

    let key = format!(
        "{:?}:{}:{}:{}",
        runtime.backend,
        runtime.topology.work_queue,
        runtime.topology.scheduled_queue,
        scope
    );
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("shared jobs registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(EmulatedJobsCoordinationRuntime::new(runtime.clone())))
        .clone()
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
        let node_id = require_non_empty("node_id", node_id.into())?;
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
        let worker_id = require_non_empty("worker_id", worker_id.into())?;
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
        let error_message = require_non_empty("job_error_message", error_message.into())?;
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

#[derive(Debug, Clone)]
pub struct JobsCoordinator {
    backend: JobsBackendAdapter,
    snapshot: JobsCoordinatorSnapshot,
}

impl JobsCoordinator {
    pub fn new(runtime: JobsRuntime) -> Self {
        let backend = JobsBackendAdapter::in_memory(&runtime);
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

    pub fn dead_letters(&self) -> &[DeadLetterOutcome] {
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
        domain: &JobsDomain,
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

    #[test]
    fn scheduler_leadership_promotes_due_jobs_once() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let mut coordinator = runtime.coordinator();
        coordinator
            .enqueue(
                JobSpec::new(
                    JobId::new("job-scheduled").unwrap(),
                    JobName::new("sitemap-regeneration").unwrap(),
                    runtime.describe().scheduled_queue.clone(),
                    "regenerate sitemap",
                )
                .unwrap()
                .scheduled_for(JobInstant::from_unix_seconds(20))
                .with_retry_policy(
                    RetryPolicy::new(2, Duration::from_secs(10), Duration::from_secs(30))
                        .unwrap()
                        .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
                )
                .with_idempotency_key(IdempotencyKey::new("sitemap-regeneration").unwrap()),
                JobInstant::from_unix_seconds(10),
            )
            .unwrap();

        let leader = coordinator
            .acquire_scheduler_leadership(
                "node-a",
                JobInstant::from_unix_seconds(12),
                Duration::from_secs(30),
            )
            .unwrap();
        assert_eq!(leader.node_id, "node-a");
        assert_eq!(coordinator.ready_jobs().len(), 0);
        assert_eq!(coordinator.scheduled_jobs().len(), 1);

        let promoted = coordinator
            .promote_due_jobs("node-a", JobInstant::from_unix_seconds(20))
            .unwrap();
        assert_eq!(promoted, vec![JobId::new("job-scheduled").unwrap()]);
        assert_eq!(coordinator.ready_jobs().len(), 1);
        assert_eq!(coordinator.scheduled_jobs().len(), 0);

        let err = coordinator
            .acquire_scheduler_leadership(
                "node-b",
                JobInstant::from_unix_seconds(25),
                Duration::from_secs(30),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            JobsModelError::LeadershipConflict {
                current_holder,
                requested_holder,
            } if current_holder == "node-a" && requested_holder == "node-b"
        ));
    }

    #[test]
    fn failed_jobs_retry_then_dead_letter_after_exhaustion() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
        let mut coordinator = runtime.coordinator();
        coordinator
            .enqueue(
                JobSpec::new(
                    JobId::new("job-retry").unwrap(),
                    JobName::new("certificate-renewal").unwrap(),
                    runtime.describe().work_queue.clone(),
                    "renew certificate",
                )
                .unwrap()
                .with_retry_policy(
                    RetryPolicy::new(2, Duration::from_secs(15), Duration::from_secs(60))
                        .unwrap()
                        .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
                )
                .with_idempotency_key(IdempotencyKey::new("certificate-renewal:v1").unwrap()),
                JobInstant::from_unix_seconds(100),
            )
            .unwrap();

        let first_lease = coordinator
            .lease_ready_jobs(
                &runtime.describe().work_queue,
                "worker-a",
                JobInstant::from_unix_seconds(100),
                Duration::from_secs(30),
                1,
            )
            .unwrap()
            .remove(0);

        let retry = coordinator
            .acknowledge_failed(
                &first_lease,
                JobInstant::from_unix_seconds(105),
                DeadLetterReason::PolicyViolation,
                "temporary upstream failure",
            )
            .unwrap();
        assert!(matches!(
            retry,
            JobFailureDisposition::Retried { ref queue, .. } if queue.as_str() == "jobs.work"
        ));
        assert_eq!(coordinator.ready_jobs().len(), 0);
        assert_eq!(coordinator.scheduled_jobs().len(), 1);

        coordinator
            .acquire_scheduler_leadership(
                "node-a",
                JobInstant::from_unix_seconds(110),
                Duration::from_secs(60),
            )
            .unwrap();
        coordinator
            .promote_due_jobs("node-a", JobInstant::from_unix_seconds(120))
            .unwrap();

        let second_lease = coordinator
            .lease_ready_jobs(
                &runtime.describe().work_queue,
                "worker-a",
                JobInstant::from_unix_seconds(120),
                Duration::from_secs(30),
                1,
            )
            .unwrap()
            .remove(0);
        let dead_letter = coordinator
            .acknowledge_failed(
                &second_lease,
                JobInstant::from_unix_seconds(125),
                DeadLetterReason::ExhaustedRetries,
                "permanent failure",
            )
            .unwrap();
        assert!(matches!(
            dead_letter,
            JobFailureDisposition::DeadLettered(_)
        ));
        assert_eq!(coordinator.dead_letters().len(), 1);
        assert_eq!(coordinator.dead_letters()[0].job_id.as_str(), "job-retry");
    }

    #[test]
    fn domain_events_dispatch_into_subscription_jobs() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let subscription = EventSubscriptionMetadata::new(
            EventSubscriptionId::new("sub-booking-email").unwrap(),
            DomainEventType::new("booking.confirmed").unwrap(),
            runtime.describe().domain_events_queue.clone(),
            EventHandlerId::new("handler-booking-email").unwrap(),
            RetryPolicy::new(2, Duration::from_secs(5), Duration::from_secs(30))
                .unwrap()
                .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
        )
        .with_idempotency_key(IdempotencyKey::new("booking.confirmed.email").unwrap());
        let handler = EventHandlerMetadata::new(
            EventHandlerId::new("handler-booking-email").unwrap(),
            "Booking email",
            runtime.describe().domain_events_queue.clone(),
            RetryPolicy::default(),
        )
        .unwrap()
        .add_subscription(subscription.clone());
        let domain = JobsDomain::new(runtime.clone())
            .add_subscription(subscription)
            .add_handler(handler);

        let envelope = DomainEventEnvelope::new(
            DomainEventId::new("evt-booking-1").unwrap(),
            DomainEventType::new("booking.confirmed").unwrap(),
            "booking",
            "booking-1",
            JobInstant::from_unix_seconds(200),
            (),
        )
        .unwrap();

        let mut coordinator = runtime.coordinator();
        let job_ids = coordinator
            .dispatch_event(&domain, &envelope, JobInstant::from_unix_seconds(200))
            .unwrap();
        assert_eq!(
            job_ids,
            vec![JobId::new("event:evt-booking-1:sub-booking-email").unwrap()]
        );
        assert_eq!(coordinator.ready_jobs().len(), 1);
        assert_eq!(
            coordinator.ready_jobs()[0].spec.queue,
            runtime.describe().domain_events_queue
        );
    }

    #[test]
    fn distributed_coordinators_do_not_share_backend_without_explicit_adapter_reuse() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let mut left = runtime.coordinator();
        let mut right = runtime.coordinator();

        left.enqueue(
            JobSpec::new(
                JobId::new("job-shared").unwrap(),
                JobName::new("shared-work").unwrap(),
                runtime.describe().work_queue.clone(),
                "shared backend work item",
            )
            .unwrap()
            .with_idempotency_key(IdempotencyKey::new("shared-work:v1").unwrap()),
            JobInstant::from_unix_seconds(10),
        )
        .unwrap();

        right.refresh();
        assert_eq!(right.ready_jobs().len(), 0);

        left.refresh();
        assert_eq!(left.ready_jobs().len(), 1);
        assert_eq!(left.ready_jobs()[0].spec.job_id.as_str(), "job-shared");
    }

    #[test]
    fn distributed_coordinators_share_backend_when_using_a_shared_adapter() {
        let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
        let adapter = JobsBackendAdapter::shared(&runtime);
        let mut left = runtime.coordinator_with_backend(adapter.clone());
        let mut right = runtime.coordinator_with_backend(adapter);

        left.enqueue(
            JobSpec::new(
                JobId::new("job-shared").unwrap(),
                JobName::new("shared-work").unwrap(),
                runtime.describe().work_queue.clone(),
                "shared backend work item",
            )
            .unwrap()
            .with_idempotency_key(IdempotencyKey::new("shared-work:v1").unwrap()),
            JobInstant::from_unix_seconds(10),
        )
        .unwrap();

        right.refresh();
        assert_eq!(right.ready_jobs().len(), 1);
        assert_eq!(right.ready_jobs()[0].spec.job_id.as_str(), "job-shared");

        let leased = right
            .lease_ready_jobs(
                &runtime.describe().work_queue,
                "worker-shared",
                JobInstant::from_unix_seconds(10),
                Duration::from_secs(30),
                1,
            )
            .unwrap();
        assert_eq!(leased.len(), 1);

        left.refresh();
        assert_eq!(left.ready_jobs().len(), 0);
        assert_eq!(left.in_flight_jobs().len(), 1);
    }
}

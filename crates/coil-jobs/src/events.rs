use crate::error::JobsModelError;
use crate::identifiers::{
    DomainEventId, DomainEventType, EventHandlerId, EventSubscriptionId, IdempotencyKey,
    JobQueueName,
};
use crate::model::JobInstant;
use crate::model::RetryPolicy;
use crate::validation::require_non_empty;

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
